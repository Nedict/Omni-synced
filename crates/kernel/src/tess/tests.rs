use std::collections::HashMap;

use solidcraft_geom::{Mesh, Vec3};

use super::*;
use crate::{Body, BoolOp};

/// Every triangle edge, with vertices merged by exact position, is shared by exactly two
/// triangles: the mesh is closed and its faces meet exactly (no cracks, no T-junctions).
fn assert_watertight(m: &Mesh, name: &str) {
    let key = |i: u32| m.positions.get(i as usize).map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
    let mut uses: HashMap<([u64; 3], [u64; 3]), i32> = HashMap::new();
    for t in &m.triangles {
        for e in 0..3 {
            let (Some(a), Some(b)) = (key(t[e]), key(t[(e + 1) % 3])) else { panic!("{name}: bad index") };
            if a == b {
                continue;
            }
            // Directed: a closed, consistently oriented mesh uses each edge once each way.
            *uses.entry((a, b)).or_default() += 1;
            *uses.entry((b, a)).or_default() -= 1;
        }
    }
    let open: Vec<_> = uses.iter().filter(|(_, n)| **n != 0).collect();
    assert!(open.is_empty(), "{name}: {} unmatched directed edges, e.g. {:?}", open.len(), open.first());
}

fn plan_body(b: &Body, tol: f64) -> (Mesh, PlanStats) {
    match plan(&b.solid, tol) {
        Ok((p, stats)) => (p.evaluate(), stats),
        Err(e) => panic!("plan: {e}"),
    }
}

fn check(name: &str, b: &Body, tol: f64, volume: f64) -> PlanStats {
    let (m, stats) = plan_body(b, tol);
    assert_watertight(&m, name);
    let got = m.measure().volume;
    // Chord error tol over the surface: the volume is short by at most about area · tol.
    let area = m.measure().area;
    assert!((got - volume).abs() <= area * tol * 1.5 + 1e-9, "{name}: volume {got} vs {volume} (area {area}, tol {tol})");
    // Outward normals: the divergence theorem gives a positive volume only if they are.
    assert!(got > 0.0, "{name}: inside out");
    let faces = b.solid.face_iter().count();
    assert_eq!(stats.planar + stats.structured + stats.delaunay, faces, "{name}: every face planned");
    stats
}

fn ok<T>(r: crate::Result<T>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn cut(a: &Body, b: &Body) -> Body {
    match crate::boolean(a, b, BoolOp::Cut) {
        Ok(Some(r)) => r,
        other => panic!("cut: {other:?}"),
    }
}

const PI: f64 = std::f64::consts::PI;

#[test]
fn primitives_are_watertight_with_the_right_volume() {
    for tol in [0.05, 0.005] {
        let s = check("box", &ok(crate::box_solid(Vec3::ZERO, Vec3::new(10.0, 6.0, 4.0))), tol, 240.0);
        assert_eq!(s, PlanStats { planar: 6, structured: 0, delaunay: 0 });
        let s = check("cylinder", &ok(crate::cylinder(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 1.0, 1.0), 2.5, 7.0)), tol, PI * 2.5 * 2.5 * 7.0);
        assert_eq!(s.delaunay, 0, "cylinder walls are structured");
        let s = check("torus", &ok(crate::torus(Vec3::ZERO, 6.0, 1.5)), tol, 2.0 * PI * PI * 6.0 * 1.5 * 1.5);
        assert_eq!(s.delaunay, 0, "torus patches are structured");
    }
}

#[test]
fn holes_and_trimmed_faces_are_watertight() {
    let tol = 0.01;
    let bx = ok(crate::box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 4.0)));
    let pin = ok(crate::cylinder(Vec3::new(5.0, 5.0, -1.0), Vec3::Z, 2.0, 6.0));
    check("box with a hole", &cut(&bx, &pin), tol, 400.0 - PI * 4.0 * 4.0);
    // A cylinder with a cross hole: trimmed cylinder halves (Delaunay) and intersection-curve walls.
    let cyl = ok(crate::cylinder(Vec3::new(5.0, 5.0, -1.0), Vec3::Z, 2.0, 6.0));
    let cross = ok(crate::cylinder(Vec3::new(5.0, -1.0, 2.0), Vec3::Y, 1.0, 12.0));
    let body = cut(&cyl, &cross);
    let (m, stats) = plan_body(&body, tol);
    assert_watertight(&m, "cross hole");
    assert!(stats.delaunay > 0, "{stats:?}");
    // Against the reference mesher's volume (no closed form for the two-cylinder intersection).
    let Ok(reference) = body.tessellate(tol * 0.2) else { panic!("reference mesh") };
    let (a, b) = (m.measure().volume, reference.measure().volume);
    assert!((a - b).abs() < 0.01 * b, "cross hole volume {a} vs {b}");
}

#[test]
fn faces_and_edges_keep_the_body_order() {
    let body = ok(crate::cylinder(Vec3::ZERO, Vec3::Z, 2.0, 5.0));
    let (m, _) = plan_body(&body, 0.01);
    let Ok(reference) = body.tessellate(0.01) else { panic!("reference mesh") };
    assert_eq!(m.edges.len(), reference.edges.len());
    assert_eq!(m.edge_faces, reference.edge_faces);
    // Same faces in the same order: each face's area agrees with the reference mesher's.
    let face_area = |m: &Mesh, f: u32| -> f64 {
        m.triangles
            .iter()
            .zip(&m.tri_face)
            .filter(|(_, tf)| **tf == f)
            .map(|(t, _)| {
                let p = |i: u32| m.positions.get(i as usize).copied().unwrap_or_default();
                (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0])).len() * 0.5
            })
            .sum()
    };
    for f in 0..body.solid.face_iter().count() as u32 {
        let (a, b) = (face_area(&m, f), face_area(&reference, f));
        assert!((a - b).abs() < 0.01 * b.max(1e-9), "face {f}: {a} vs {b}");
    }
    // Each edge polyline ends on the same points as the reference's.
    for (a, b) in m.edges.iter().zip(&reference.edges) {
        assert!(a.first().zip(b.first()).is_some_and(|(p, q)| p.dist(*q) < 1e-9));
        assert!(a.last().zip(b.last()).is_some_and(|(p, q)| p.dist(*q) < 1e-9));
    }
}

/// Side-by-side wireframes, reference mesher (left) and plan (right):
/// `SOLIDCRAFT_TESS_SVG=<dir> cargo test -p solidcraft-kernel render_wireframes -- --ignored`.
#[test]
#[ignore]
fn render_wireframes() {
    let Ok(dir) = std::env::var("SOLIDCRAFT_TESS_SVG") else { return };
    let tol = 0.02;
    let bx = ok(crate::box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 4.0)));
    let pin = ok(crate::cylinder(Vec3::new(5.0, 5.0, -1.0), Vec3::Z, 2.0, 6.0));
    let cross = ok(crate::cylinder(Vec3::new(5.0, -1.0, 2.0), Vec3::Y, 1.0, 12.0));
    let filleted =
        ok(crate::fillet(&bx, &[Vec3::new(5.0, 0.0, 4.0), Vec3::new(10.0, 5.0, 4.0), Vec3::new(5.0, 10.0, 4.0), Vec3::new(0.0, 5.0, 4.0)], 1.5));
    let shapes = [
        ("box_hole", cut(&bx, &pin)),
        ("cylinder", pin.clone()),
        ("cross_hole", cut(&pin, &cross)),
        ("torus", ok(crate::torus(Vec3::ZERO, 6.0, 1.5))),
        ("fillet", filleted),
    ];
    for (name, body) in shapes {
        let t0 = std::time::Instant::now();
        let Ok(old) = body.tessellate(tol) else { panic!("reference") };
        let t_old = t0.elapsed().as_secs_f64() * 1e3;
        let t0 = std::time::Instant::now();
        let new = match plan(&body.solid, tol) {
            Ok((p, st)) => Some((p.evaluate(), st)),
            Err(e) => {
                println!("{name}: no plan ({e})");
                None
            }
        };
        let t_new = t0.elapsed().as_secs_f64() * 1e3;
        let mut svg = String::from("<svg xmlns='http://www.w3.org/2000/svg' width='1600' height='860' style='background:#fff'>");
        let label = |x: f64, text: String| format!("<text x='{x}' y='40' font-family='sans-serif' font-size='26'>{text}</text>");
        svg += &label(20.0, format!("meshalgo (Delaunay): {} tris, {t_old:.1} ms", old.triangles.len()));
        svg += &draw(&old, 0.0);
        if let Some((m, st)) = &new {
            svg += &label(
                820.0,
                format!("plan: {} tris, {t_new:.1} ms  [{} planar, {} grid, {} cdt]", m.triangles.len(), st.planar, st.structured, st.delaunay),
            );
            svg += &draw(m, 800.0);
        }
        svg += "</svg>";
        let _ = std::fs::write(format!("{dir}/{name}.svg"), svg);
    }
}

/// Isometric, painter-sorted, flat-shaded triangles with their edges, in an 800×800 panel.
fn draw(m: &Mesh, x0: f64) -> String {
    let (c30, s30) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let proj = |p: Vec3| -> (f64, f64, f64) {
        // Turn 45° about Z, tilt 30° about X; depth along the view.
        let (x, y) = ((p.x - p.y) * std::f64::consts::FRAC_1_SQRT_2, (p.x + p.y) * std::f64::consts::FRAC_1_SQRT_2);
        (x, p.z * c30 - y * s30, y * c30 + p.z * s30)
    };
    let pts: Vec<(f64, f64, f64)> = m.positions.iter().map(|p| proj(*p)).collect();
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for p in &pts {
        lo = (lo.0.min(p.0), lo.1.min(p.1));
        hi = (hi.0.max(p.0), hi.1.max(p.1));
    }
    let k = 700.0 / (hi.0 - lo.0).max(hi.1 - lo.1).max(1e-9);
    let sx = |p: (f64, f64, f64)| (x0 + 50.0 + (p.0 - lo.0) * k, 800.0 - (p.1 - lo.1) * k);
    let mut tris: Vec<(f64, [u32; 3])> = m.triangles.iter().map(|t| (t.iter().map(|&i| pts[i as usize].2).sum::<f64>(), *t)).collect();
    tris.sort_by(|a, b| b.0.total_cmp(&a.0));
    let light = Vec3::new(0.3, -0.5, 0.8).normalized().unwrap_or(Vec3::new(0.0, 0.0, 1.0));
    let mut out = String::new();
    for (_, t) in tris {
        let [a, b, c] = t.map(|i| m.positions[i as usize]);
        let n = (b - a).cross(c - a).normalized().unwrap_or(Vec3::new(0.0, 0.0, 1.0));
        let shade = (90.0 + 150.0 * n.dot(light).max(0.0)) as u8;
        let pp: Vec<String> = t
            .iter()
            .map(|&i| {
                let (x, y) = sx(pts[i as usize]);
                format!("{x:.1},{y:.1}")
            })
            .collect();
        out += &format!(
            "<polygon points='{}' fill='rgb({shade},{shade},{})' stroke='#1d3557' stroke-width='0.5'/>",
            pp.join(" "),
            shade.saturating_add(20)
        );
    }
    out
}
