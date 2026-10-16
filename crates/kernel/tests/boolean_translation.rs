//! Deterministic regression for #1649, outside the nightly-only property-test binary.

use solidcraft_geom::Vec3;
use solidcraft_kernel::{Body, BoolOp, boolean, cylinder, measure, transform};

fn moved(body: &Body, offset: Vec3) -> Body {
    transform(body, offset, Vec3::ZERO, Vec3::Z, 0.0).expect("translate")
}

fn reference_union_volume() -> f64 {
    // At height z, the overlap is a rectangle: the positive half-width of
    // the X cylinder in y times the Y cylinder's positive half-width in x,
    // clipped to the first cylinder's 10 mm length. Integrate these analytic
    // cross-sections with midpoint quadrature, independently of the B-rep/mesh.
    let steps = 100_000;
    let dz = 10.0 / f64::from(steps);
    let overlap: f64 = (0..steps)
        .map(|i| {
            let z = 2.5 + (f64::from(i) + 0.5) * dz;
            let y = (25.0 - (z - 7.5).powi(2)).max(0.0).sqrt();
            let x = (156.25 - (z - 15.0).powi(2)).max(0.0).sqrt().min(10.0);
            x * y * dz
        })
        .sum();
    std::f64::consts::PI * (5.0_f64.powi(2) * 10.0 + 12.5_f64.powi(2) * 5.0) - overlap
}

#[test]
fn crossing_cylinders_union_moved_far() {
    let a = cylinder(Vec3::new(17.5, 30.0, 7.5), Vec3::X, 5.0, 10.0).unwrap();
    let b = cylinder(Vec3::new(17.5, 30.0, 15.0), Vec3::Y, 12.5, 5.0).unwrap();
    let expected = reference_union_volume();
    let union = boolean(&a, &b, BoolOp::Union).unwrap().unwrap();
    let base = measure(&union).unwrap();
    let slack = expected * 2e-3;
    assert!((base.volume - expected).abs() < slack, "near origin: {} vs {expected}", base.volume);

    for offset in [Vec3::new(1000.0, -700.0, 300.0), Vec3::new(-1000.0, 700.0, -300.0), Vec3::new(1e5, -7e4, 3e4)] {
        // Recompute after moving the operands, and separately move an existing result.
        // The latter isolates measurement from the boolean's numerical choices.
        let recomputed = boolean(&moved(&a, offset), &moved(&b, offset), BoolOp::Union).unwrap().unwrap();
        for result in [&recomputed, &moved(&union, offset)] {
            assert!(result.validity().is_empty(), "{offset:?}: {:?}", result.validity());
            let m = measure(result).unwrap();
            assert!((m.volume - expected).abs() < slack, "{offset:?}: {} vs reference {expected}", m.volume);
            assert!((m.volume - base.volume).abs() < slack, "{offset:?}: {} vs origin {}", m.volume, base.volume);
            assert!(m.centroid.dist(base.centroid + offset) < 0.05, "{offset:?}: centroid {:?} vs {:?}", m.centroid, base.centroid + offset);
            assert!((m.bbox.min - offset).dist(base.bbox.min) < 0.01);
            assert!((m.bbox.max - offset).dist(base.bbox.max) < 0.01);
        }
    }
}
