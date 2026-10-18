//! Issue #2258 (robustness seed 61): a shelled arched block, whose wall wraps round an earlier
//! cylinder cut along X through its curved end, cut again by the same cylinder 5 mm along Y.
//! The second cut failed ("no result").

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use solidcraft_geom::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};

use crate::{BoolOp, boolean, cylinder, extrude, measure, shell};

/// The sketch: a 47.5 × 20 rectangle closed at x = 0 by the arc through (-12.5, 10).
fn arched() -> Region2 {
    // The circle through (0, 20), (-12.5, 10) and (0, 0).
    let center = Vec2::new(-2.25, 10.0);
    let radius = 10.25;
    let start = (20.0f64 - 10.0).atan2(0.0 - -2.25);
    let sweep = std::f64::consts::TAU - 2.0 * start;
    let segs = vec![
        Seg2::Line { a: Vec2::new(0.0, 0.0), b: Vec2::new(47.5, 0.0) },
        Seg2::Line { a: Vec2::new(47.5, 0.0), b: Vec2::new(47.5, 20.0) },
        Seg2::Line { a: Vec2::new(47.5, 20.0), b: Vec2::new(0.0, 20.0) },
        Seg2::Arc { center, radius, start, sweep },
    ];
    Region2 { outer: Loop2 { segs }, holes: vec![] }
}

#[test]
fn second_cut_beside_a_shelled_cut_builds() {
    let block = extrude(&Plane::XY, &[arched()], 0.0, 7.5).unwrap().remove(0);
    let tool = |y: f64| cylinder(Vec3::new(-12.5, y, 2.5), Vec3::X, 10.0, 20.0).unwrap();
    let cut = boolean(&block, &tool(-0.2286902286902297), BoolOp::Cut).unwrap().unwrap();
    let shelled = shell(&cut, &[Vec3::new(47.5, 10.0, 3.75)], 2.0).unwrap();
    let before = measure(&shelled).unwrap();
    assert!((before.volume - 4906.63).abs() < 0.5, "{before:?}");
    let r = boolean(&shelled, &tool(4.77130977130977), BoolOp::Cut).unwrap().expect("the second cut builds");
    let m = measure(&r).unwrap();
    assert!(m.volume.is_finite() && m.volume > 3000.0 && m.volume < before.volume - 1.0, "{m:?}");
    assert_eq!(r.solid.boundaries().len(), 1, "{m:?}");
    assert!(r.solid.boundaries().iter().all(|s| s.extract_boundaries().is_empty()), "{m:?}");
}
