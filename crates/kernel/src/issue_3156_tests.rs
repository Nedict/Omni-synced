//! A revolve ended flush with a face of the body it joins or cuts (#3156): a profile turned ±90° about an axis lying in a
//! plate's top face has both its end faces in that face's plane. The faces are pushed apart exactly, as an extrude's are; the
//! tool is not nudged. Expected volumes are analytic: the plate, plus or minus the overlap of the revolve with it.
#![allow(clippy::unwrap_used, clippy::panic)]
use crate::*;
use solidcraft_geom::{Loop2, Plane, Region2, Vec2, Vec3};
use std::f64::consts::PI;

/// A 10 x 100 x 6 plate, x -50 to -40, y -50 to 50, z 41 to 47.
fn plate() -> Body {
    box_solid(Vec3::new(-50.0, -50.0, 41.0), Vec3::new(-40.0, 50.0, 47.0)).unwrap()
}

/// A half turn of a profile 30 to 37 from the axis y = 0, z = 47 along x (in the plate's top face), from +y round under the
/// axis to −y: both end faces lie in z = 47, over the plate. `width(r)` is the profile's width along x at distance r,
/// centred on x = −45 (a rectangle 4 wide, or a trapezoid 4 wide at 30 and 2 at 37).
fn half_turn(trapezoid: bool) -> Body {
    let plane = Plane::new(Vec3::new(0.0, 0.0, 47.0), Vec3::X, Vec3::Y).unwrap();
    let top = if trapezoid { 1.0 } else { 2.0 };
    let outline = Loop2::polygon(&[Vec2::new(-47.0, 30.0), Vec2::new(-43.0, 30.0), Vec2::new(-45.0 + top, 37.0), Vec2::new(-45.0 - top, 37.0)]);
    let body = revolve(&plane, &[Region2 { outer: outline, holes: vec![] }], Vec2::ZERO, Vec2::X, -PI).unwrap().pop().unwrap();
    let m = body.tessellate_with(0.01, true).unwrap().bounds();
    assert!((m.min.z - 10.0).abs() < 1e-6 && (m.max.z - 47.0).abs() < 1e-6, "the turn goes under the axis: {m:?}");
    body
}

fn width(trapezoid: bool, r: f64) -> f64 {
    if trapezoid { 4.0 - 2.0 * (r - 30.0) / 7.0 } else { 4.0 }
}

/// ∫ f over [a, b] (Simpson's rule, 2000 steps: smooth integrands, far below the tolerances asked).
fn integral(f: impl Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    let n = 2000;
    let h = (b - a) / f64::from(n);
    let inner: f64 = (1..n).map(|i| f(a + h * f64::from(i)) * if i % 2 == 1 { 4.0 } else { 2.0 }).sum();
    (f(a) + f(b) + inner) * h / 3.0
}

/// The revolve's volume (Pappus, ring by ring), and the part of it inside the plate: on each side of the axis, the ring of
/// radius r lies in the plate for the angle asin(6 / r) below the top face.
fn volumes(trapezoid: bool) -> (f64, f64) {
    let turn = PI * integral(|r| width(trapezoid, r) * r, 30.0, 37.0);
    let inside = 2.0 * integral(|r| width(trapezoid, r) * r * (6.0 / r).asin(), 30.0, 37.0);
    (turn, inside)
}

/// The volume on a mesh 0.002 mm from the faces: within 1e-4 of the analytic volumes here (`measure`'s coarser meshes are not).
fn volume(b: &Body) -> f64 {
    b.tessellate_with(0.002, true).unwrap().measure().volume
}

fn exact(how: &BooleanDiagnostics) -> bool {
    !how.events.iter().any(|e| e.strategy.alters_result())
}

#[test]
fn a_revolve_joined_flush_with_the_face_its_axis_lies_in_is_exact() {
    for trapezoid in [false, true] {
        let tool = half_turn(trapezoid);
        let (turn, inside) = volumes(trapezoid);
        let tol = 1e-4 * turn;
        assert!((volume(&tool) - turn).abs() < tol, "trapezoid {trapezoid}: {} vs {turn}", volume(&tool));
        let (r, how) = capture_boolean_diagnostics(|| boolean(&plate(), &tool, BoolOp::Union));
        let r = r.unwrap().unwrap();
        assert!(exact(&how) && how.warning().is_none(), "trapezoid {trapezoid}: {how:?}");
        let v = volume(&r);
        assert!((v - (6000.0 + turn - inside)).abs() < tol, "trapezoid {trapezoid}: {v} vs {}", 6000.0 + turn - inside);
        assert!(r.lumps().unwrap().len() == 1 && r.validity().is_empty(), "trapezoid {trapezoid}: {:?}", r.validity());
        // Where the end faces were, the top face is whole: nothing sticks out of z = 47 or was left a hair below it.
        let b = r.tessellate_with(0.01, true).unwrap().bounds();
        assert!((b.max.z - 47.0).abs() < 1e-9 && (b.min.z - 10.0).abs() < 1e-6, "trapezoid {trapezoid}: {b:?}");
    }
}

#[test]
fn a_revolve_cut_flush_with_the_face_its_axis_lies_in_is_exact() {
    for trapezoid in [false, true] {
        let tool = half_turn(trapezoid);
        let (turn, inside) = volumes(trapezoid);
        let (r, how) = capture_boolean_diagnostics(|| boolean(&plate(), &tool, BoolOp::Cut));
        let r = r.unwrap().unwrap();
        assert!(exact(&how) && how.warning().is_none(), "trapezoid {trapezoid}: {how:?}");
        let v = volume(&r);
        assert!((v - (6000.0 - inside)).abs() < 1e-4 * turn, "trapezoid {trapezoid}: {v} vs {}", 6000.0 - inside);
        // The groove goes through the plate, but only 4 of its 10 across: it stays one piece.
        assert!(r.lumps().unwrap().len() == 1 && r.validity().is_empty(), "trapezoid {trapezoid}: {:?}", r.validity());
    }
}
