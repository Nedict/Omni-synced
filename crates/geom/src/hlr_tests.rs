use super::*;
use crate::brep::{Brep, Curve, Edge, EdgeUse, Face, Shell, Surface};

/// An axis-aligned box with line edges and plane faces.
fn cuboid(lo: [f64; 3], hi: [f64; 3]) -> Brep {
    let corner = |i: usize| [if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }];
    let vertices: Vec<[f64; 3]> = (0..8).map(corner).collect();
    let pairs = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
    let edges: Vec<Edge> =
        pairs.iter().map(|&(a, b)| Edge { vertices: [a as u32, b as u32], curve: Curve::Line { start: corner(a), end: corner(b) } }).collect();
    let cycle = |vs: [usize; 4]| -> Vec<EdgeUse> {
        (0..4)
            .map(|k| {
                let (a, b) = (vs[k], vs[(k + 1) % 4]);
                let (i, fwd) = pairs
                    .iter()
                    .enumerate()
                    .find_map(|(i, &(x, y))| (x == a && y == b).then_some((i, true)).or((x == b && y == a).then_some((i, false))))
                    .unwrap();
                EdgeUse { edge: i as u32, forward: fwd }
            })
            .collect()
    };
    let plane = |o: usize, u: usize, v: usize| {
        let (o, u, v) = (corner(o), corner(u), corner(v));
        Surface::Plane { origin: o, u_axis: [u[0] - o[0], u[1] - o[1], u[2] - o[2]], v_axis: [v[0] - o[0], v[1] - o[1], v[2] - o[2]] }
    };
    let faces = vec![
        Face { loops: vec![cycle([0, 2, 3, 1])], surface: plane(0, 2, 1), forward: true },
        Face { loops: vec![cycle([4, 5, 7, 6])], surface: plane(4, 5, 6), forward: true },
        Face { loops: vec![cycle([0, 1, 5, 4])], surface: plane(0, 1, 4), forward: true },
        Face { loops: vec![cycle([2, 6, 7, 3])], surface: plane(2, 6, 3), forward: true },
        Face { loops: vec![cycle([0, 4, 6, 2])], surface: plane(0, 4, 2), forward: true },
        Face { loops: vec![cycle([1, 3, 7, 5])], surface: plane(1, 3, 5), forward: true },
    ];
    Brep { version: crate::brep::VERSION, shells: vec![Shell { vertices, edges, faces }] }
}

fn identity(body: usize, t: [f64; 3]) -> Placement {
    Placement { body, rotation: [Vec3::new(1., 0., 0.), Vec3::new(0., 1., 0.), Vec3::new(0., 0., 1.)], translation: Vec3::new(t[0], t[1], t[2]) }
}

fn never() -> bool {
    false
}

#[test]
fn iso_cube_hides_the_three_back_edges() {
    let def = prepare(&cuboid([0.; 3], [1.; 3]), &Options::default()).unwrap();
    let view = View::new(Vec3::new(-1., -1., -1.), Vec3::new(0., 0., 1.)).unwrap();
    let d = hidden_lines(&[def], &[identity(0, [0.; 3])], &view, &Options::default(), &never).unwrap();
    let mut hidden = Vec::new();
    for l in &d.lines {
        let Source::Edge { edge, .. } = l.source else { panic!("a box has no silhouettes") };
        let states: Vec<_> = l.pieces.iter().map(|p| p.visibility).collect();
        assert!(states.windows(2).all(|w| w[0] == w[1]), "edge {edge} is split: {states:?}");
        if states[0] == Visibility::Hidden {
            hidden.push(edge);
        }
    }
    hidden.sort_unstable();
    // The edges at the corner nearest the origin: (0,1), (0,2), (0,4).
    assert_eq!(hidden, vec![0, 4, 8], "{d:#?}");
}

#[test]
fn a_box_in_front_hides_the_middle_of_an_edge() {
    let back = prepare(&cuboid([0.; 3], [1.; 3]), &Options::default()).unwrap();
    let front = prepare(&cuboid([0.5, -2., 0.25], [1.5, -1., 0.75]), &Options::default()).unwrap();
    let view = View::new(Vec3::new(0., 1., 0.), Vec3::new(0., 0., 1.)).unwrap();
    let d = hidden_lines(&[back, front], &[identity(0, [0.; 3]), identity(1, [0.; 3])], &view, &Options::default(), &never).unwrap();
    // Edge 9 joins corners 1 (1,0,0) and 5 (1,0,1): t is z.
    let line = d.lines.iter().find(|l| l.placement == 0 && l.source == Source::Edge { edge: 9, tangent: false }).unwrap();
    let got: Vec<([f64; 2], Visibility)> = line.pieces.iter().map(|p| (p.t, p.visibility)).collect();
    assert_eq!(got.len(), 3, "{got:?}");
    assert_eq!(got[1].1, Visibility::Hidden);
    assert!((got[1].0[0] - 0.25).abs() < 1e-12 && (got[1].0[1] - 0.75).abs() < 1e-12, "{got:?}");
    assert_eq!((got[0].1, got[2].1), (Visibility::Visible, Visibility::Visible));
}

#[test]
fn quadratic_roots_with_no_linear_term() {
    // A ray from inside a sphere, square to the radius at its start (b = 0): both roots.
    let mut roots = surface::quadratic(1., 0., -100.);
    roots.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(roots.len(), 2, "{roots:?}");
    assert!((roots[0].0 + 10.).abs() < 1e-12 && (roots[1].0 - 10.).abs() < 1e-12, "{roots:?}");
    let mut roots = surface::quadratic(1., -0., -100.);
    roots.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!((roots[0].0 + 10.).abs() < 1e-12 && (roots[1].0 - 10.).abs() < 1e-12, "{roots:?}");
}

#[test]
fn a_torus_seen_along_its_axis_has_both_equator_silhouettes() {
    // A whole-meridian torus record: the outer equator sits where the closed profile's ends meet.
    let frame = crate::brep::Frame { origin: [0.; 3], x_axis: [1., 0., 0.], y_axis: [0., 1., 0.] };
    let tau = std::f64::consts::TAU;
    let s = surface::S3::new(&Surface::Torus { frame, major_radius: 20., minor_radius: 5., longitude: [0., tau], meridian: [0., tau] }).unwrap();
    let mut radii: Vec<f64> = s
        .rev_silhouette(Vec3::new(0., 0., -1.))
        .iter()
        .map(|(c, _)| match c {
            curve::C3::Arc { r, .. } => r[0],
            _ => panic!("circles expected"),
        })
        .collect();
    radii.sort_by(f64::total_cmp);
    assert_eq!(radii.len(), 2, "{radii:?}");
    assert!((radii[0] - 15.).abs() < 1e-9 && (radii[1] - 25.).abs() < 1e-9, "{radii:?}");
}

#[test]
fn far_apart_bodies_do_not_merge_nearby_visible_edges() {
    // A 5 mm cube and a box 100 m away: coincidence is judged at the chords' own accuracy,
    // not at a fraction of the scene's size, so none of the cube's edges is taken as a duplicate.
    let near = prepare(&cuboid([0.; 3], [5.; 3]), &Options::default()).unwrap();
    let far = prepare(&cuboid([100_000.; 3], [100_001.; 3]), &Options::default()).unwrap();
    let view = View::new(Vec3::new(-1., -0.7, -0.4), Vec3::new(0., 0., 1.)).unwrap();
    let d = hidden_lines(&[near, far], &[identity(0, [0.; 3]), identity(1, [0.; 3])], &view, &Options::default(), &never).unwrap();
    let covered_visible = d
        .lines
        .iter()
        .filter(|l| l.placement == 0)
        .flat_map(|l| l.pieces.iter())
        .filter(|p| p.visibility == Visibility::Visible && p.covered)
        .count();
    assert_eq!(covered_visible, 0);
}

#[test]
fn a_thin_torus_tube_stops_a_ray() {
    // R 100, r 0.1: a ray across the tube meets it twice within 0.2 mm.
    let roots = surface::torus_roots(Vec3::new(100., -50., 0.), Vec3::new(0., 1., 0.), Vec3::new(0., 0., 1.), 100., 0.1, [0., 200.]);
    assert!(roots.iter().filter(|(_, grazing)| !grazing).count() >= 2, "{roots:?}");
}

#[test]
fn a_thin_tube_on_a_huge_torus_stops_a_ray() {
    // R 100 m, r 1 mm: inside the tube the implicit function stays far below a size^4 floor.
    // A radial ray from 5 mm inside the centre circle crosses the tube at 4 and 6 mm.
    let roots = surface::torus_roots(Vec3::new(100_000. - 5., 0., 0.), Vec3::new(1., 0., 0.), Vec3::new(0., 0., 1.), 100_000., 1., [0., 10.]);
    let hits: Vec<f64> = roots.iter().filter(|(_, grazing)| !grazing).map(|(l, _)| *l).collect();
    // The implicit form cancels terms of 4e20 here: the roots are good to about 1e-6 mm.
    assert!(hits.len() == 2 && (hits[0] - 4.).abs() < 1e-5 && (hits[1] - 6.).abs() < 1e-5, "{roots:?}");
}

#[test]
fn a_hair_thin_tube_on_a_kilometre_torus_stops_a_ray() {
    // R 1 km, r 0.1 mm (R/r = 1e7): roots at 0.9 and 1.1 mm, good to about 1e-4 mm here.
    let roots = surface::torus_roots(Vec3::new(1e6 - 1., 0., 0.), Vec3::new(1., 0., 0.), Vec3::new(0., 0., 1.), 1e6, 0.1, [0., 2.]);
    let hits: Vec<f64> = roots.iter().filter(|(_, grazing)| !grazing).map(|(l, _)| *l).collect();
    assert!(hits.len() == 2 && (hits[0] - 0.9).abs() < 1e-3 && (hits[1] - 1.1).abs() < 1e-3, "{roots:?}");
}

#[test]
fn a_ray_from_the_hole_crosses_a_thin_large_torus() {
    // R 4 m, r 0.01 mm, from the centre outward over 8 m: only the shell around the tube is sampled.
    let roots = surface::torus_roots(Vec3::new(0., 0., 0.), Vec3::new(1., 0., 0.), Vec3::new(0., 0., 1.), 4000., 0.01, [0., 8000.]);
    let hits: Vec<f64> = roots.iter().filter(|(_, grazing)| !grazing).map(|(l, _)| *l).collect();
    assert!(hits.len() == 2 && (hits[0] - 3999.99).abs() < 1e-6 && (hits[1] - 4000.01).abs() < 1e-6, "{roots:?}");
}

#[test]
fn a_torus_seen_nearly_along_its_axis_keeps_its_silhouettes() {
    // Meridian starting off the equator and a view 1e-4 rad from the axis: the silhouette lives
    // in narrow profile intervals around the equators, which must not fall between samples.
    let frame = crate::brep::Frame { origin: [0.; 3], x_axis: [1., 0., 0.], y_axis: [0., 1., 0.] };
    let tau = std::f64::consts::TAU;
    let shift = std::f64::consts::PI / 720.;
    let s =
        surface::S3::new(&Surface::Torus { frame, major_radius: 1000., minor_radius: 500., longitude: [0., tau], meridian: [shift, shift + tau] })
            .unwrap();
    let d = Vec3::new(1e-4, 0., -1.).normalized().unwrap();
    let curves = s.rev_silhouette(d);
    // Both the outer (radius about 1500) and the inner (about 500) silhouettes are there.
    let radii: Vec<f64> = curves
        .iter()
        .flat_map(|(c, _)| {
            let [t0, t1] = c.domain();
            (0..=8).map(move |i| c.at(t0 + (t1 - t0) * i as f64 / 8.))
        })
        .map(|p| p.x.hypot(p.y))
        .collect();
    assert!(radii.iter().any(|r| *r > 1400.) && radii.iter().any(|r| *r < 600.), "{radii:?}");
}

#[test]
fn a_shallow_cut_through_a_torus_tube_is_found() {
    // R 20, r 5, a ray along the axis 0.01 mm inside the outer equator: a 0.63 mm chord.
    let roots = surface::torus_roots(Vec3::new(24.99, 0., -10.), Vec3::new(0., 0., 1.), Vec3::new(0., 0., 1.), 20., 5., [0., 20.]);
    let hits: Vec<f64> = roots.iter().filter(|(_, grazing)| !grazing).map(|(l, _)| *l).collect();
    let half = (25f64 - 4.99f64.powi(2)).sqrt();
    assert!(hits.len() == 2 && (hits[0] - (10. - half)).abs() < 1e-6 && (hits[1] - (10. + half)).abs() < 1e-6, "{roots:?}");
}

/// A frame at `o` with x along +X and y along +Y (z along +Z).
fn frame_at(o: [f64; 3]) -> crate::brep::Frame {
    crate::brep::Frame { origin: o, x_axis: [1., 0., 0.], y_axis: [0., 1., 0.] }
}

/// A full circle about Z through vertex `v`.
fn circle(o: [f64; 3], r: f64, v: u32) -> Edge {
    Edge { vertices: [v, v], curve: Curve::CircleArc { frame: frame_at(o), radius: r, angle: [0., std::f64::consts::TAU] } }
}

fn uses(edge: u32, forward: bool) -> EdgeUse {
    EdgeUse { edge, forward }
}

/// A cylinder of radius `r` and height `h` standing on the XY plane, as analytic records.
fn cylinder(r: f64, h: f64) -> Brep {
    turned(r, r, h)
}

/// A cylinder (`r0 == r1`) or a cone frustum from radius `r0` at z = 0 to `r1` at z = `h`,
/// its seam at angle 0.
fn turned(r0: f64, r1: f64, h: f64) -> Brep {
    let tau = std::f64::consts::TAU;
    let seam = Edge { vertices: [0, 1], curve: Curve::Line { start: [r0, 0., 0.], end: [r1, 0., h] } };
    let edges = vec![circle([0.; 3], r0, 0), circle([0., 0., h], r1, 1), seam];
    let side = if r0 == r1 {
        Surface::Cylinder { frame: frame_at([0.; 3]), radius: r0, angle: [0., tau], height: [0., h] }
    } else {
        Surface::Cone { frame: frame_at([0.; 3]), radius_at_origin: r0, half_angle: ((r1 - r0) / h).atan(), angle: [0., tau], height: [0., h] }
    };
    let faces = vec![
        Face { loops: vec![vec![uses(0, true), uses(2, true), uses(1, false), uses(2, false)]], surface: side, forward: true },
        Face {
            loops: vec![vec![uses(0, false)]],
            surface: Surface::Plane { origin: [0.; 3], u_axis: [1., 0., 0.], v_axis: [0., -1., 0.] },
            forward: true,
        },
        Face {
            loops: vec![vec![uses(1, true)]],
            surface: Surface::Plane { origin: [0., 0., h], u_axis: [1., 0., 0.], v_axis: [0., 1., 0.] },
            forward: true,
        },
    ];
    Brep { version: crate::brep::VERSION, shells: vec![Shell { vertices: vec![[r0, 0., 0.], [r1, 0., h]], edges, faces }] }
}

/// A sphere of radius `r` at the origin: two hemispheres meeting at the equator.
fn sphere(r: f64) -> Brep {
    let (tau, half) = (std::f64::consts::TAU, std::f64::consts::FRAC_PI_2);
    let hemisphere = |latitude: [f64; 2], forward| Face {
        loops: vec![vec![uses(0, forward)]],
        surface: Surface::Sphere { frame: frame_at([0.; 3]), radius: r, longitude: [0., tau], latitude },
        forward: true,
    };
    let faces = vec![hemisphere([0., half], true), hemisphere([-half, 0.], false)];
    Brep { version: crate::brep::VERSION, shells: vec![Shell { vertices: vec![[r, 0., 0.]], edges: vec![circle([0.; 3], r, 0)], faces }] }
}

/// A torus about Z (major radius `big`, minor `small`): top and bottom halves meeting at the
/// outer and inner equators.
fn torus(big: f64, small: f64) -> Brep {
    let (tau, pi) = (std::f64::consts::TAU, std::f64::consts::PI);
    let half = |meridian: [f64; 2], forward: bool| Face {
        loops: vec![vec![uses(0, forward)], vec![uses(1, !forward)]],
        surface: Surface::Torus { frame: frame_at([0.; 3]), major_radius: big, minor_radius: small, longitude: [0., tau], meridian },
        forward: true,
    };
    let edges = vec![circle([0.; 3], big + small, 0), circle([0.; 3], big - small, 1)];
    let faces = vec![half([0., pi], true), half([pi, tau], false)];
    Brep { version: crate::brep::VERSION, shells: vec![Shell { vertices: vec![[big + small, 0., 0.], [big - small, 0., 0.]], edges, faces }] }
}

/// A bilinear (twisted) spline patch over x, y in [-5, 5] with z between 0 and 1.
fn twisted_patch() -> Brep {
    let p = [[-5., -5., 0.], [5., -5., 1.], [5., 5., 0.], [-5., 5., 1.]];
    let edges =
        (0..4).map(|i| Edge { vertices: [i as u32, ((i + 1) % 4) as u32], curve: Curve::Line { start: p[i], end: p[(i + 1) % 4] } }).collect();
    let face = Face {
        loops: vec![(0..4).map(|e| uses(e, true)).collect()],
        surface: Surface::Spline { u_knots: vec![0., 0., 1., 1.], v_knots: vec![0., 0., 1., 1.], points: vec![vec![p[0], p[3]], vec![p[1], p[2]]] },
        forward: true,
    };
    Brep { version: crate::brep::VERSION, shells: vec![Shell { vertices: p.to_vec(), edges, faces: vec![face] }] }
}

fn pieces(d: &Drawing) -> impl Iterator<Item = &Piece> {
    d.lines.iter().flat_map(|l| l.pieces.iter())
}

fn silhouette_pieces(d: &Drawing) -> Vec<&Piece> {
    d.lines.iter().filter(|l| matches!(l.source, Source::Silhouette { .. })).flat_map(|l| l.pieces.iter()).collect()
}

fn edge_line(d: &Drawing, placement: usize, edge: usize) -> &Line {
    let found = d.lines.iter().find(|l| l.placement == placement && matches!(l.source, Source::Edge { edge: e, .. } if e == edge));
    found.unwrap_or_else(|| panic!("no line for edge {edge} of placement {placement}"))
}

#[test]
fn a_cylinder_seen_from_the_side_has_exact_silhouette_lines() {
    let def = prepare(&cylinder(10., 30.), &Options::default()).unwrap();
    // Slightly from above, along +X (rulings at +-90 degrees) and along +Y (rulings at 0 and
    // 180 degrees: one runs along the seam, which is not drawn as an edge).
    for dir in [Vec3::new(1., 0., -0.3), Vec3::new(0., 1., -0.3)] {
        let view = View::new(dir, Vec3::new(0., 0., 1.)).unwrap();
        let d = hidden_lines(std::slice::from_ref(&def), &[identity(0, [0.; 3])], &view, &Options::default(), &never).unwrap();
        check_cylinder_sides(&d, &view);
    }
}

fn check_cylinder_sides(d: &Drawing, view: &View) {
    let sil = silhouette_pieces(d);
    assert!(!sil.is_empty());
    let rise = 30. * view.y.z;
    let mut sides = Vec::new();
    for p in sil {
        assert_eq!(p.accuracy, CurveAccuracy::Analytic);
        assert_eq!(p.visibility, Visibility::Visible);
        for g in &p.geometry {
            let Seg2::Line { a, b } = *g else { panic!("a silhouette ruling is a line: {g:?}") };
            assert!((a.x.abs() - 10.).abs() < 1e-12 && (a.x - b.x).abs() < 1e-12, "{g:?}");
            sides.push((a.x.signum(), (b.y - a.y).abs()));
        }
    }
    for side in [-1., 1.] {
        let len: f64 = sides.iter().filter(|s| s.0 == side).map(|s| s.1).sum();
        assert!((len - rise).abs() < 1e-9, "side {side}: {len} vs {rise}");
    }
    assert_eq!(d.stats.uncertain_uncovered, 0, "{:?}", d.stats);
    assert!(d.unsupported.is_empty());
}

#[test]
fn a_sphere_outline_is_exact_conic_arcs_in_any_view() {
    let def = prepare(&sphere(10.), &Options::default()).unwrap();
    let views = [
        (Vec3::new(1., 2., 3.), Vec3::new(0., 0., 1.)),
        (Vec3::new(0., 0., -1.), Vec3::new(0., 1., 0.)),
        (Vec3::new(-0.3, 1., 0.1), Vec3::new(0., 0., 1.)),
    ];
    for (dir, up) in views {
        let view = View::new(dir, up).unwrap();
        let d = hidden_lines(std::slice::from_ref(&def), &[identity(0, [0.; 3])], &view, &Options::default(), &never).unwrap();
        // The outline: every uncovered visible piece lies on the circle of radius 10.
        let mut drawn = 0.;
        for p in
            pieces(&d).filter(|p| p.visibility == Visibility::Visible && !p.covered && p.geometry.iter().all(|g| matches!(g, Seg2::Conic { .. })))
        {
            assert_eq!(p.accuracy, CurveAccuracy::Analytic);
            for g in &p.geometry {
                let ends = (g.point_at(0.), g.point_at(1.));
                if (ends.0.len() - 10.).abs() > 1e-9 {
                    continue;
                }
                for k in 0..=16 {
                    let q = g.point_at(k as f64 / 16.);
                    assert!((q.len() - 10.).abs() < 1e-9, "{dir:?}: {q:?} off the outline");
                }
                drawn += ends.0.dist(ends.1);
            }
        }
        assert!(drawn > 40., "{dir:?}: the outline is drawn ({drawn} mm of chords)");
        for p in silhouette_pieces(&d) {
            assert!(p.geometry.iter().all(|g| matches!(g, Seg2::Conic { .. })), "{dir:?}: {p:?}");
        }
        assert_eq!(d.stats.uncertain_uncovered, 0, "{dir:?}: {:?}", d.stats);
    }
}

#[test]
fn a_torus_seen_face_on_is_drawn_at_both_equators() {
    let def = prepare(&torus(20., 5.), &Options::default()).unwrap();
    let view = View::new(Vec3::new(0., 0., -1.), Vec3::new(0., 1., 0.)).unwrap();
    let d = hidden_lines(&[def], &[identity(0, [0.; 3])], &view, &Options::default(), &never).unwrap();
    let (mut outer, mut inner) = (false, false);
    for p in pieces(&d).filter(|p| p.visibility == Visibility::Visible && !p.covered) {
        for g in &p.geometry {
            for k in 0..=8 {
                let r = g.point_at(k as f64 / 8.).len();
                assert!((r - 25.).abs() < 1e-9 || (r - 15.).abs() < 1e-9, "a point at radius {r}");
                outer |= (r - 25.).abs() < 1e-9;
                inner |= (r - 15.).abs() < 1e-9;
            }
        }
    }
    assert!(outer && inner);
    // Both equators are silhouettes along the seams between the halves: one curve each.
    assert_eq!(d.stats.silhouettes, 2, "{:?}", d.stats);
    assert_eq!(d.stats.uncertain_uncovered, 0, "{:?}", d.stats);
}

#[test]
fn an_edge_under_a_torus_is_hidden_where_the_tube_covers_it() {
    // Looking down on a torus (R 20, r 5) over a long bar: the bar's top edge along y = 1 is
    // hidden where sqrt(x^2 + 1) is between 15 and 25.
    let t = prepare(&torus(20., 5.), &Options::default()).unwrap();
    let bar = prepare(&cuboid([-40., -1., -12.], [40., 1., -10.]), &Options::default()).unwrap();
    let view = View::new(Vec3::new(0., 0., -1.), Vec3::new(0., 1., 0.)).unwrap();
    let d = hidden_lines(&[t, bar], &[identity(0, [0.; 3]), identity(1, [0.; 3])], &view, &Options::default(), &never).unwrap();
    // Edge 3 joins corners 6 (-40, 1, -10) and 7 (40, 1, -10): x = 80 t - 40.
    let hidden: Vec<[f64; 2]> =
        edge_line(&d, 1, 3).pieces.iter().filter(|p| p.visibility == Visibility::Hidden).map(|p| [p.t[0] * 80. - 40., p.t[1] * 80. - 40.]).collect();
    let (a, b) = (224f64.sqrt(), 624f64.sqrt());
    assert_eq!(hidden.len(), 2, "{hidden:?}");
    for (h, want) in hidden.iter().zip([[-b, -a], [a, b]]) {
        assert!((h[0] - want[0]).abs() < 1e-9 && (h[1] - want[1]).abs() < 1e-9, "{hidden:?}");
    }
    assert_eq!(d.stats.uncertain_uncovered, 0, "{:?}", d.stats);
}

#[test]
fn a_part_far_from_the_origin_draws_the_same_as_at_the_origin() {
    let def = prepare(&cylinder(10., 30.), &Options::default()).unwrap();
    let view = View::new(Vec3::new(-1., -0.7, -0.4), Vec3::new(0., 0., 1.)).unwrap();
    let defs = [def];
    let states = |d: &Drawing| -> Vec<(Visibility, bool, usize)> { pieces(d).map(|p| (p.visibility, p.covered, p.geometry.len())).collect() };
    let base = hidden_lines(&defs, &[identity(0, [0.; 3])], &view, &Options::default(), &never).unwrap();
    for t in [[1e5, 2e5, -3e5], [1e7, 2e7, -3e7]] {
        let d = hidden_lines(&defs, &[identity(0, t)], &view, &Options::default(), &never).unwrap();
        assert_eq!(states(&base), states(&d), "classification moved at {t:?}");
        let shift = view.screen(Vec3::new(t[0], t[1], t[2]));
        // Only the final shift back to the caller's frame rounds: about 1e-16 of |t|.
        let tol = 1e-15 * (t[0].abs() + t[1].abs() + t[2].abs()) + 1e-9;
        for (a, b) in pieces(&base).zip(pieces(&d)) {
            for (g, h) in a.geometry.iter().zip(&b.geometry) {
                for s in [0., 0.5, 1.] {
                    let (p, q) = (g.point_at(s), h.point_at(s));
                    assert!((q.x - shift[0] - p.x).abs() < tol && (q.y - shift[1] - p.y).abs() < tol, "{p:?} vs {q:?} at {t:?}");
                }
            }
        }
    }
}

#[test]
fn freeform_faces_are_reported_and_never_hide_for_certain() {
    let patch = prepare(&twisted_patch(), &Options::default()).unwrap();
    assert_eq!(patch.report.freeform_faces, vec![0]);
    // A bar under the patch, seen from above: its top edges (2, 3, 6, 7) are behind the patch only.
    let bar = prepare(&cuboid([-2., -1., -5.], [2., 1., -3.]), &Options::default()).unwrap();
    let view = View::new(Vec3::new(0.1, 0.05, -1.), Vec3::new(0., 1., 0.)).unwrap();
    let defs = [patch, bar];
    let scene = [identity(0, [0.; 3]), identity(1, [0.; 3])];
    let top = |d: &Drawing| -> Vec<Visibility> {
        [2, 3, 6, 7].iter().flat_map(|&e| edge_line(d, 1, e).pieces.iter().map(|p| p.visibility)).collect::<Vec<_>>()
    };
    let d = hidden_lines(&defs, &scene, &view, &Options::default(), &never).unwrap();
    assert_eq!(d.unsupported, vec![Unsupported { placement: 0, face: 0, reason: "freeform face" }]);
    assert!(top(&d).iter().all(|v| *v == Visibility::Uncertain), "{:?}", top(&d));
    // With freeform faces handled the patch hides them.
    let d = hidden_lines(&defs, &scene, &view, &Options { freeform: true, ..Options::default() }, &never).unwrap();
    assert!(d.unsupported.is_empty());
    assert!(top(&d).iter().all(|v| *v == Visibility::Hidden), "{:?}", top(&d));
}

#[test]
fn mirrored_placements_bad_indices_and_bad_views_are_refused() {
    let def = prepare(&cuboid([0.; 3], [1.; 3]), &Options::default()).unwrap();
    let view = View::new(Vec3::new(-1., -1., -1.), Vec3::new(0., 0., 1.)).unwrap();
    let mirror =
        Placement { body: 0, rotation: [Vec3::new(-1., 0., 0.), Vec3::new(0., 1., 0.), Vec3::new(0., 0., 1.)], translation: Vec3::new(0., 0., 0.) };
    let defs = [def];
    assert!(matches!(hidden_lines(&defs, &[mirror], &view, &Options::default(), &never), Err(HlrError::Invalid(_))));
    assert!(matches!(hidden_lines(&defs, &[identity(3, [0.; 3])], &view, &Options::default(), &never), Err(HlrError::Invalid(_))));
    assert!(matches!(hidden_lines(&defs, &[identity(0, [f64::NAN, 0., 0.])], &view, &Options::default(), &never), Err(HlrError::Invalid(_))));
    assert!(View::new(Vec3::new(0., 0., 0.), Vec3::new(0., 0., 1.)).is_err());
    assert!(View::new(Vec3::new(0., 0., 1.), Vec3::new(0., 0., 2.)).is_err());
}

#[test]
fn cancellation_and_budgets_return_errors() {
    let def = prepare(&cylinder(10., 30.), &Options::default()).unwrap();
    let view = View::new(Vec3::new(-1., -0.7, -0.4), Vec3::new(0., 0., 1.)).unwrap();
    let defs = [def];
    let scene = [identity(0, [0.; 3]), identity(0, [5., 0., 0.])];
    assert!(matches!(hidden_lines(&defs, &scene, &view, &Options::default(), &|| true), Err(HlrError::Cancelled)));
    assert!(matches!(hidden_lines(&defs, &scene, &view, &Options { max_rays: 3, ..Options::default() }, &never), Err(HlrError::Budget(_))));
    assert!(matches!(hidden_lines(&defs, &scene, &view, &Options { max_tracks: 1, ..Options::default() }, &never), Err(HlrError::Budget(_))));
}

#[test]
fn two_touching_boxes_draw_each_shared_edge_once() {
    let a = prepare(&cuboid([0.; 3], [10.; 3]), &Options::default()).unwrap();
    let b = prepare(&cuboid([10., 0., 0.], [20., 10., 10.]), &Options::default()).unwrap();
    let view = View::new(Vec3::new(-1., -0.7, -0.5), Vec3::new(0., 0., 1.)).unwrap();
    let d = hidden_lines(&[a, b], &[identity(0, [0.; 3]), identity(1, [0.; 3])], &view, &Options::default(), &never).unwrap();
    assert_eq!(d.stats.uncertain_uncovered, 0, "{:?}", d.stats);
    // The edges at x = 10: 5, 7, 9, 11 of the first box are 4, 6, 8, 10 of the second.
    let uncovered = |l: &Line| -> f64 { l.pieces.iter().filter(|p| !p.covered).map(|p| p.t[1] - p.t[0]).sum() };
    for (ea, eb) in [(5, 4), (7, 6), (9, 8), (11, 10)] {
        let once = uncovered(edge_line(&d, 0, ea)) + uncovered(edge_line(&d, 1, eb));
        assert!((once - 1.).abs() < 1e-9, "edges {ea}/{eb} drawn {once} times");
    }
}

#[test]
fn a_cone_in_a_front_view_has_exact_outline_rulings_and_edge_on_circles() {
    // A frustum from radius 5 at z = 0 to 10 at z = 20, seen exactly from the front: the
    // rulings run from (+-5, 0) to (+-10, 20) (one along the seam) and each circle is drawn
    // once as the segment it is seen edge-on as.
    let def = prepare(&turned(5., 10., 20.), &Options::default()).unwrap();
    let view = View::new(Vec3::new(0., 1., 0.), Vec3::new(0., 0., 1.)).unwrap();
    let d = hidden_lines(&[def], &[identity(0, [0.; 3])], &view, &Options::default(), &never).unwrap();
    assert_eq!(d.stats.uncertain_uncovered, 0, "{:?}", d.stats);
    let mut rulings: Vec<[f64; 4]> = Vec::new();
    for p in silhouette_pieces(&d) {
        assert_eq!((p.visibility, p.accuracy), (Visibility::Visible, CurveAccuracy::Analytic));
        for g in &p.geometry {
            let Seg2::Line { a, b } = *g else { panic!("a ruling is a line: {g:?}") };
            let (a, b) = if a.y <= b.y { (a, b) } else { (b, a) };
            rulings.push([a.x, a.y, b.x, b.y]);
        }
    }
    rulings.sort_by(|a, b| a[0].total_cmp(&b[0]));
    assert_eq!(rulings.len(), 2, "{rulings:?}");
    for (r, want) in rulings.iter().zip([[-5., 0., -10., 20.], [5., 0., 10., 20.]]) {
        assert!(r.iter().zip(want).all(|(x, y)| (x - y).abs() < 1e-9), "{rulings:?}");
    }
    for (edge, (y, half)) in [(0, (0., 5.)), (1, (20., 10.))] {
        let drawn: f64 = edge_line(&d, 0, edge)
            .pieces
            .iter()
            .filter(|p| p.visibility == Visibility::Visible && !p.covered)
            .flat_map(|p| p.geometry.iter())
            .map(|g| {
                let (a, b) = (g.point_at(0.), g.point_at(1.));
                assert!((a.y - y).abs() < 1e-9 && (b.y - y).abs() < 1e-9 && a.x.abs() <= half + 1e-9, "{g:?}");
                (b.x - a.x).abs()
            })
            .sum();
        assert!((drawn - 2. * half).abs() < 1e-6, "circle {edge}: {drawn} mm drawn");
    }
}

#[test]
fn a_torus_seen_obliquely_spans_its_outer_diameter() {
    // Across the view (screen x is square to the axis here) the outline of a torus reaches
    // R + r on both sides, whatever the tilt; the traced silhouette reports its deviation.
    let def = prepare(&torus(20., 5.), &Options::default()).unwrap();
    for tilt in [0.3, 1., 3.] {
        let view = View::new(Vec3::new(0., 1., -tilt), Vec3::new(0., 0., 1.)).unwrap();
        let d = hidden_lines(std::slice::from_ref(&def), &[identity(0, [0.; 3])], &view, &Options::default(), &never).unwrap();
        assert_eq!(d.stats.uncertain_uncovered, 0, "tilt {tilt}: {:?}", d.stats);
        let mut dev = 0f64;
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for p in pieces(&d).filter(|p| p.visibility == Visibility::Visible && !p.covered) {
            if let CurveAccuracy::Sampled { max_sample_deviation_mm } = p.accuracy {
                dev = dev.max(max_sample_deviation_mm);
            }
            for g in &p.geometry {
                for s in [0., 0.5, 1.] {
                    let q = g.point_at(s);
                    (lo, hi) = (lo.min(q.x), hi.max(q.x));
                }
            }
        }
        assert!(dev < 1e-3, "tilt {tilt}: deviation {dev}");
        assert!((hi - 25.).abs() < 1e-3 && (lo + 25.).abs() < 1e-3, "tilt {tilt}: x from {lo} to {hi}");
    }
}

/// A disc bounded by a rational quadratic circle of radius `r` (control points `pts` with
/// weights 1 and 1/sqrt 2 alternating) whose first three quarters are squeezed into the
/// parameters 0 to 0.009.
fn squeezed_disc(r: f64, pts: [[f64; 2]; 9]) -> Brep {
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let homogeneous = pts.iter().enumerate().map(|(i, p)| if i % 2 == 1 { [p[0] * s, p[1] * s, 0., s] } else { [p[0], p[1], 0., 1.] }).collect();
    let knots = vec![0., 0., 0., 0.003, 0.003, 0.006, 0.006, 0.009, 0.009, 1., 1., 1.];
    Brep {
        version: crate::brep::VERSION,
        shells: vec![Shell {
            vertices: vec![[r, 0., 0.]],
            edges: vec![Edge { vertices: [0, 0], curve: Curve::RationalSpline { knots, homogeneous } }],
            faces: vec![Face {
                loops: vec![vec![uses(0, true)]],
                surface: Surface::Plane { origin: [0.; 3], u_axis: [1., 0., 0.], v_axis: [0., 1., 0.] },
                forward: true,
            }],
        }],
    }
}

fn circle_points(r: f64) -> [[f64; 2]; 9] {
    [[r, 0.], [r, r], [0., r], [-r, r], [-r, 0.], [-r, -r], [0., -r], [r, -r], [r, 0.]]
}

#[test]
fn an_unevenly_parameterised_circle_is_drawn_whole() {
    // The conic arcs must still turn the full 360 degrees.
    let def = prepare(&squeezed_disc(10., circle_points(10.)), &Options::default()).unwrap();
    let view = View::new(Vec3::new(0.2, 0.1, -1.), Vec3::new(0., 1., 0.)).unwrap();
    let d = hidden_lines(&[def], &[identity(0, [0.; 3])], &view, &Options::default(), &never).unwrap();
    let mut sweep = 0.;
    for p in pieces(&d) {
        assert_eq!((p.visibility, p.accuracy), (Visibility::Visible, CurveAccuracy::Analytic));
        for g in &p.geometry {
            let Seg2::Conic { w, .. } = *g else { panic!("a circle is drawn as conic arcs: {g:?}") };
            sweep += 2. * w.acos();
        }
    }
    assert!((sweep - std::f64::consts::TAU).abs() < 1e-9, "the arcs turn {sweep} rad");
}

#[test]
fn a_spline_off_its_circle_in_a_narrow_knot_span_is_not_taken_for_one() {
    // Moving one control point of the squeezed circle bends the span 0.003 to 0.006 off the
    // circle by about 0.3 mm: the edge is drawn as sampled chords, not exact arcs.
    let mut pts = circle_points(10.);
    if let Some(p) = pts.get_mut(3) {
        *p = [-11., 10.];
    }
    let def = prepare(&squeezed_disc(10., pts), &Options::default()).unwrap();
    let view = View::new(Vec3::new(0.2, 0.1, -1.), Vec3::new(0., 1., 0.)).unwrap();
    let d = hidden_lines(&[def], &[identity(0, [0.; 3])], &view, &Options::default(), &never).unwrap();
    assert!(pieces(&d).all(|p| matches!(p.accuracy, CurveAccuracy::Sampled { .. })), "{d:?}");
}

/// A cone frustum about a vertical axis through `o`: radius `r0` at the frame's origin and
/// slope `k`, between the heights `h`.
fn frustum(o: [f64; 3], r0: f64, k: f64, h: [f64; 2]) -> Shell {
    let tau = std::f64::consts::TAU;
    let at = |z: f64| [o[0] + r0 + k * z, o[1], o[2] + z];
    let ring = |z: f64, v: u32| circle([o[0], o[1], o[2] + z], r0 + k * z, v);
    let edges = vec![ring(h[0], 0), ring(h[1], 1), Edge { vertices: [0, 1], curve: Curve::Line { start: at(h[0]), end: at(h[1]) } }];
    let plane = |z: f64, flip: f64| Surface::Plane { origin: [o[0], o[1], o[2] + z], u_axis: [1., 0., 0.], v_axis: [0., flip, 0.] };
    let faces = vec![
        Face {
            loops: vec![vec![uses(0, true), uses(2, true), uses(1, false), uses(2, false)]],
            surface: Surface::Cone { frame: frame_at(o), radius_at_origin: r0, half_angle: k.atan(), angle: [0., tau], height: h },
            forward: true,
        },
        Face { loops: vec![vec![uses(0, false)]], surface: plane(h[0], -1.), forward: true },
        Face { loops: vec![vec![uses(1, true)]], surface: plane(h[1], 1.), forward: true },
    ];
    Shell { vertices: vec![at(h[0]), at(h[1])], edges, faces }
}

#[test]
fn nearly_coaxial_cones_far_out_keep_their_own_silhouettes() {
    // Two frusta of slope 0.25 whose axes are slightly apart, far out: recognition may take
    // them for one cone, but each is drawn with its own outline. Small ones 0.005 mm apart
    // 1e7 mm out, and 1 km ones 0.4 mm apart 1e9 mm out.
    for (x, r, gap) in [(1e7, 5., 0.005), (1e9, 1e6, 0.4)] {
        let shells = vec![frustum([x, 0., 0.], r, 0.25, [0., 10.]), frustum([x + gap, 0., 0.], r, 0.25, [20., 30.])];
        check_two_cone_outlines(&Brep { version: crate::brep::VERSION, shells });
    }
}

fn check_two_cone_outlines(body: &Brep) {
    let def = prepare(body, &Options::default()).unwrap();
    let view = View::new(Vec3::new(0., 1., 0.), Vec3::new(0., 0., 1.)).unwrap();
    let d = hidden_lines(&[def], &[identity(0, [0.; 3])], &view, &Options::default(), &never).unwrap();
    for (z0, z1) in [(0., 10.), (20., 30.)] {
        let rulings = silhouette_pieces(&d)
            .iter()
            .filter(|p| p.visibility == Visibility::Visible && !p.covered)
            .flat_map(|p| p.geometry.iter())
            .filter(|g| {
                let (a, b) = (g.point_at(0.), g.point_at(1.));
                (a.y.min(b.y) - z0).abs() < 1e-6 && (a.y.max(b.y) - z1).abs() < 1e-6
            })
            .count();
        assert_eq!(rulings, 2, "z {z0}..{z1}: {:?}", silhouette_pieces(&d));
    }
}
