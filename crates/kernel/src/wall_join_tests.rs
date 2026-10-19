//! A wall joined round a part, flush with its sides: a ring whose hole is the part's outline,
//! rounded corners and all (a box's walls round its gridded floor), standing on the part's
//! bottom or on a level below it. The bodies only touch, back to back on planes and cylinders.

use std::f64::consts::{FRAC_PI_2, PI};

use solidcraft_geom::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};

use crate::*;

/// The rectangle from `lo` to `hi` with its corners rounded to `r`, counter-clockwise.
fn rounded(lo: Vec2, hi: Vec2, r: f64) -> Loop2 {
    let line = |a: Vec2, b: Vec2| Seg2::Line { a, b };
    let arc = |center: Vec2, start: f64| Seg2::Arc { center, radius: r, start, sweep: FRAC_PI_2 };
    Loop2 {
        segs: vec![
            line(Vec2::new(lo.x + r, lo.y), Vec2::new(hi.x - r, lo.y)),
            arc(Vec2::new(hi.x - r, lo.y + r), -FRAC_PI_2),
            line(Vec2::new(hi.x, lo.y + r), Vec2::new(hi.x, hi.y - r)),
            arc(Vec2::new(hi.x - r, hi.y - r), 0.0),
            line(Vec2::new(hi.x - r, hi.y), Vec2::new(lo.x + r, hi.y)),
            arc(Vec2::new(lo.x + r, hi.y - r), FRAC_PI_2),
            line(Vec2::new(lo.x, hi.y - r), Vec2::new(lo.x, lo.y + r)),
            arc(Vec2::new(lo.x + r, lo.y + r), PI),
        ],
    }
}

/// The area of [`rounded`].
fn rounded_area(lo: Vec2, hi: Vec2, r: f64) -> f64 {
    (hi.x - lo.x) * (hi.y - lo.y) - (4.0 - PI) * r * r
}

const LO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
const HI: Vec2 = Vec2 { x: 60.0, y: 50.0 };

/// A 60 × 50 plate 3.25 thick with r4 corners, a pocket with sloping sides in its top and a
/// socket under it (so that it is no prism, and nothing below its bottom is clear).
fn part() -> Body {
    let plate = extrude(&Plane::XY, &[Region2 { outer: rounded(LO, HI, 4.0), holes: vec![] }], 0.0, 3.25).unwrap().pop().unwrap();
    let pocket = Region2 { outer: rounded(Vec2::new(10.0, 10.0), Vec2::new(50.0, 40.0), 3.0), holes: vec![] };
    let top = Plane::from_normal(Vec3::new(0.0, 0.0, 4.0), Vec3::Z).unwrap();
    let pocket = extrude_tapered(&top, &pocket, 3.0, -1.0, -0.3).unwrap();
    let plate = boolean(&plate, &pocket, BoolOp::Cut).unwrap().unwrap();
    let socket = box_solid(Vec3::new(12.0, 12.0, -4.75), Vec3::new(48.0, 38.0, 0.0)).unwrap();
    boolean(&plate, &socket, BoolOp::Union).unwrap().unwrap()
}

/// The 10 mm wall round the part, r14 outside, from `z0` to `z1`.
fn wall(z0: f64, z1: f64) -> (Body, f64) {
    let (outer, inner) = ((Vec2::new(-10.0, -10.0), Vec2::new(70.0, 60.0), 14.0), (LO, HI, 4.0));
    let ring = Region2 { outer: rounded(outer.0, outer.1, outer.2), holes: vec![rounded(inner.0, inner.1, inner.2).reversed()] };
    let area = rounded_area(outer.0, outer.1, outer.2) - rounded_area(inner.0, inner.1, inner.2);
    (extrude(&Plane::XY, &[ring], z0, z1).unwrap().pop().unwrap(), area * (z1 - z0))
}

/// From the part's bottom, and from the socket's floor past its top: joined exactly (glued where
/// they touch, no tool nudged), one solid of both volumes, with no face left where they touched.
#[test]
fn a_wall_joins_flush_round_a_part_with_rounded_corners() {
    let part = part();
    let v_part = measure(&part).unwrap().volume;
    for (z0, z1) in [(0.0, 40.0), (-4.75, 5.25)] {
        let (wall, v_wall) = wall(z0, z1);
        for (a, b) in [(&part, &wall), (&wall, &part)] {
            let (r, d) = capture_boolean_diagnostics(|| boolean(a, b, BoolOp::Union));
            let r = r.unwrap().unwrap();
            assert!(!d.events.iter().any(|e| e.strategy.alters_result()), "{z0}..{z1}: an exact join: {d:?}");
            assert_eq!(r.shell_count(), 1, "{z0}..{z1}");
            assert!(r.validity().is_empty(), "{z0}..{z1}: {:?}", r.validity());
            let v = measure(&r).unwrap().volume;
            // Measured to 2e-5 where faces are curved (as the drain pads are).
            assert!((v - v_part - v_wall).abs() < 2e-5 * (v_part + v_wall), "{z0}..{z1}: {v} vs {}", v_part + v_wall);
            let size = a.size().max(b.size());
            assert!(crate::coplanar::slit_free(&r, a, b, size * 1e-4), "{z0}..{z1}: a face left where the bodies touched");
        }
    }
}

/// A contact far smaller than the face it lies on (a tall wall beside a thin plate) is seen
/// whichever body is asked about first: the wall's face meshes into triangles none of whose
/// samples fall on the plate.
#[test]
fn a_contact_much_smaller_than_its_face_is_seen() {
    let plate = box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 1.0)).unwrap();
    let wall = box_solid(Vec3::new(10.0, 0.0, 0.0), Vec3::new(12.0, 10.0, 40.0)).unwrap();
    assert!(crate::coplanar::faces_touch(&plate, &wall));
    assert!(crate::coplanar::faces_touch(&wall, &plate));
    let apart = box_solid(Vec3::new(10.5, 0.0, 0.0), Vec3::new(12.0, 10.0, 40.0)).unwrap();
    assert!(!crate::coplanar::faces_touch(&plate, &apart));
}

/// Two blocks side by side in one body, not fused, have a slit where they touch; the same blocks
/// joined have none.
#[test]
fn a_face_left_where_bodies_touched_is_a_slit() {
    let a = box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0)).unwrap();
    let b = box_solid(Vec3::new(10.0, 0.0, 0.0), Vec3::new(20.0, 10.0, 10.0)).unwrap();
    let shells: Vec<_> = a.solid.boundaries().iter().chain(b.solid.boundaries()).cloned().collect();
    let side_by_side = Body::new(crate::body::Solid::try_new(shells).unwrap()).unwrap();
    assert!(!crate::coplanar::slit_free(&side_by_side, &a, &b, 0.01));
    let joined = boolean(&a, &b, BoolOp::Union).unwrap().unwrap();
    assert!(crate::coplanar::slit_free(&joined, &a, &b, 0.01));
}

/// A plane square to a round side's axis splits it along an exact arc of its circle, half
/// turns included, where a curve fitted to the face's mesh would stray by the mesh's sag.
#[test]
fn a_round_side_is_split_across_its_axis_on_its_circle() {
    let (c, r) = (Vec3::new(5.0, -3.0, 0.0), 7.0);
    let body = cylinder(c, Vec3::Z, r, 12.0).unwrap();
    let sides: Vec<usize> = (0..body.face_count()).filter(|i| matches!(face_surface(&body, *i), Some(FaceSurface::Cylinder { .. }))).collect();
    assert!(!sides.is_empty());
    let plane = Plane::from_normal(Vec3::new(0.0, 0.0, 4.5), Vec3::Z).unwrap();
    let (split, _) = crate::splitface::split_across(&body, &sides, plane).unwrap();
    assert!(split.validity().is_empty(), "{:?}", split.validity());
    use truck_modeling::{BoundedCurve, ParametricCurve};
    let at = |p: truck_modeling::Point3| crate::body::from_p3(p);
    let cuts: Vec<_> =
        split.solid.edge_iter().filter(|e| (at(e.front().point()).z - 4.5).abs() < 1e-9 && (at(e.back().point()).z - 4.5).abs() < 1e-9).collect();
    assert!(!cuts.is_empty());
    for e in cuts {
        let curve = e.curve();
        let (t0, t1) = curve.range_tuple();
        for k in 0..=16 {
            let p = at(curve.subs(t0 + (t1 - t0) * f64::from(k) / 16.0));
            let off = Vec3::new(p.x - c.x, p.y - c.y, 0.0).len();
            assert!((off - r).abs() < 1e-9 && (p.z - 4.5).abs() < 1e-9, "{p:?} is {} off the circle", off - r);
        }
    }
}
