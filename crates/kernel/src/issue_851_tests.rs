//! Fillets and chamfers where a curved wall runs out into a flat face in a knife edge (#851).
use crate::*;
use solidcraft_geom::Vec3;
use std::f64::consts::PI;

const R_WALL: f64 = 21.0;
const HEIGHT: f64 = 10.0;
/// x of the knife edge: the arc wall's far end.
const TIP_X: f64 = 62.0 + 1.0 / 6.0;

/// A 41⅙ × 21 plate plus the corner a circle of radius 21 leaves, extruded 10 mm: the arc wall
/// meets the front face square and runs out into the back face (y = 0) in a knife edge at (TIP_X, 0).
fn part() -> Body {
    let x0 = TIP_X - R_WALL;
    let plate = box_solid(Vec3::new(0.0, -R_WALL, 0.0), Vec3::new(x0, 0.0, HEIGHT)).unwrap();
    let corner = box_solid(Vec3::new(x0, -R_WALL, 0.0), Vec3::new(TIP_X, 0.0, HEIGHT)).unwrap();
    let disc = cylinder(Vec3::new(TIP_X, -R_WALL, 0.0), Vec3::Z, R_WALL, HEIGHT).unwrap();
    let corner = boolean(&corner, &disc, BoolOp::Cut).unwrap().unwrap();
    boolean(&plate, &corner, BoolOp::Union).unwrap().unwrap()
}

/// Area a ball of radius `r` takes off the cusp: the quadrilateral tip–wall-contact–wall-centre–
/// ball-centre-contact, less the wall's sector and the ball's.
fn removed_area(r: f64) -> f64 {
    let beta = ((R_WALL - r) / (R_WALL + r)).acos();
    let o = (TIP_X - (4.0 * R_WALL * r).sqrt(), -r);
    let c = (TIP_X, -R_WALL);
    let tip = (TIP_X, 0.0);
    let t1 = (o.0, 0.0);
    // Shoelace over t1 → tip → c → o.
    let pts = [t1, tip, c, o];
    let quad: f64 = (0..4).map(|i| pts[i].0 * pts[(i + 1) % 4].1 - pts[(i + 1) % 4].0 * pts[i].1).sum::<f64>().abs() / 2.0;
    quad - 0.5 * R_WALL * R_WALL * beta - 0.5 * r * r * (PI - beta)
}

fn knife() -> Vec3 {
    Vec3::new(TIP_X, 0.0, HEIGHT / 2.0)
}

/// The same part with the corner joined first: its faces come in another order, so the plane
/// and the wall swap sides at the knife edge.
fn part_other_order() -> Body {
    let x0 = TIP_X - R_WALL;
    let plate = box_solid(Vec3::new(0.0, -R_WALL, 0.0), Vec3::new(x0, 0.0, HEIGHT)).unwrap();
    let corner = box_solid(Vec3::new(x0, -R_WALL, 0.0), Vec3::new(TIP_X, 0.0, HEIGHT)).unwrap();
    let disc = cylinder(Vec3::new(TIP_X, -R_WALL, 0.0), Vec3::Z, R_WALL, HEIGHT).unwrap();
    let both = boolean(&corner, &plate, BoolOp::Union).unwrap().unwrap();
    boolean(&both, &disc, BoolOp::Cut).unwrap().unwrap()
}

/// The same solid as one box less the wall's cylinder: the plane comes before the wall in the
/// shell, so the plane is side 1 at the knife edge.
fn part_cut_from_a_box() -> Body {
    let slab = box_solid(Vec3::new(0.0, -R_WALL, 0.0), Vec3::new(TIP_X, 0.0, HEIGHT)).unwrap();
    let disc = cylinder(Vec3::new(TIP_X, -R_WALL, 0.0), Vec3::Z, R_WALL, HEIGHT).unwrap();
    boolean(&slab, &disc, BoolOp::Cut).unwrap().unwrap()
}

/// Area a chamfer of distance `d` takes off the cusp: the triangle between the plane's contact,
/// the tip and the wall's contact, less the wall's circular segment.
fn chamfered_area(d: f64) -> f64 {
    let beta = ((R_WALL - d) / (R_WALL + d)).acos();
    let o = (TIP_X - (4.0 * R_WALL * d).sqrt(), -d);
    let c = (TIP_X, -R_WALL);
    let k = R_WALL / (R_WALL + d);
    let q2 = (c.0 + (o.0 - c.0) * k, c.1 + (o.1 - c.1) * k);
    let (t1, tip) = ((o.0, 0.0), (TIP_X, 0.0));
    let triangle = 0.5 * ((tip.0 - t1.0) * (q2.1 - t1.1) - (q2.0 - t1.0) * (tip.1 - t1.1)).abs();
    triangle - 0.5 * R_WALL * R_WALL * (beta - beta.sin())
}

#[test]
fn fillet_the_knife_edge() {
    for (name, body) in [("plate first", part()), ("corner first", part_other_order()), ("cut from a box", part_cut_from_a_box())] {
        let before = measure(&body).unwrap().volume;
        for r in [0.5, 2.0, 5.0] {
            let rounded = fillet(&body, &[knife()], r).unwrap_or_else(|e| panic!("{name} r={r}: {e}"));
            assert!(rounded.validity().is_empty(), "{name} r={r}: {:?}", rounded.validity());
            assert_eq!(rounded.face_count(), body.face_count() + 1, "{name} r={r}: one blend face");
            let v = measure(&rounded).unwrap().volume;
            let expected = before - HEIGHT * removed_area(r);
            assert!((v - expected).abs() < 1e-3 * expected.abs(), "{name} r={r}: volume {v}, expected {expected}");
        }
    }
}

#[test]
fn chamfer_the_knife_edge() {
    for (name, body) in [("plate first", part()), ("corner first", part_other_order()), ("cut from a box", part_cut_from_a_box())] {
        let before = measure(&body).unwrap().volume;
        for d in [0.5, 2.0, 5.0] {
            let cut = chamfer(&body, &[knife()], d).unwrap_or_else(|e| panic!("{name} d={d}: {e}"));
            assert!(cut.validity().is_empty(), "{name} d={d}: {:?}", cut.validity());
            assert_eq!(cut.face_count(), body.face_count() + 1, "{name} d={d}: one chamfer face");
            let v = measure(&cut).unwrap().volume;
            let expected = before - HEIGHT * chamfered_area(d);
            assert!((v - expected).abs() < 1e-3 * expected.abs(), "{name} d={d}: volume {v}, expected {expected}");
        }
    }
}

/// A ball that does not fit between the faces, and sizes at the ends of the range, are refused
/// or blended; they never panic or give an invalid solid.
#[test]
fn knife_edge_sizes_never_panic() {
    let body = part();
    for r in [1e-5, 1e-3, 10.0, 21.0, 40.0, 1e3, 1e6 - 1.0] {
        for round in [true, false] {
            let made = if round { fillet(&body, &[knife()], r) } else { chamfer(&body, &[knife()], r) };
            if let Ok(b) = made {
                assert!(b.validity().is_empty(), "r={r}: {:?}", b.validity());
            }
        }
    }
}

/// Mirrored parts reverse the edge and its faces' use of it: the blend is the same.
#[test]
fn knife_edge_blends_the_same_in_a_mirror() {
    let body = part();
    let reference = measure(&fillet(&body, &[knife()], 2.0).unwrap()).unwrap().volume;
    let mirror = |axis: usize| {
        let mut m = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        m[axis][axis] = -1.0;
        m
    };
    for (axis, at) in [(0, Vec3::new(-TIP_X, 0.0, 5.0)), (1, Vec3::new(TIP_X, 0.0, 5.0)), (2, Vec3::new(TIP_X, 0.0, -5.0))] {
        let mirrored = transform_matrix(&body, mirror(axis)).unwrap();
        let v = measure(&fillet(&mirrored, &[at], 2.0).unwrap()).unwrap().volume;
        assert!((v - reference).abs() < 1e-6, "mirror {axis}: volume {v}, expected {reference}");
    }
}
