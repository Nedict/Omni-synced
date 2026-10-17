//! #1510: a body split by a plane that a face of the body lies in: the parts meet in the plane
//! and together are the body.

use solidcraft_geom::{Plane, Vec3};

use crate::{Body, BoolOp, build::box_solid, build::cylinder, ops::boolean, ops::split_by_plane};

fn cut(a: &Body, t: crate::Result<Body>) -> Body {
    boolean(a, &t.unwrap(), BoolOp::Cut).unwrap().unwrap()
}

/// A 2.5 mm plate with a through hole of radius 2.5 and a groove 4 mm wide and 1.5 deep from
/// the bottom, running from the hole's centre to the edge: the groove's ceiling lies at z = 1.5
/// and its boundary runs into the hole.
fn plate(width: f64) -> Body {
    let p = box_solid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 20.0, 2.5)).unwrap();
    let p = cut(&p, cylinder(Vec3::new(20.0, 10.0, -1.0), Vec3::Z, 2.5, 5.0));
    cut(&p, box_solid(Vec3::new(20.0, 10.0 - width / 2.0, -1.0), Vec3::new(41.0, 10.0 + width / 2.0, 1.5)))
}

#[test]
fn a_split_in_the_plane_of_a_face_keeps_all_of_the_body() {
    for width in [4.0, 5.0] {
        let b = plate(width);
        let v = b.volume_at(1e-3).unwrap();
        let plane = Plane::from_normal(Vec3::new(0.0, 0.0, 1.5), Vec3::Z).unwrap();
        let (parts, d) = crate::capture_boolean_diagnostics(|| split_by_plane(&b, &plane));
        let parts = parts.unwrap();
        assert_eq!(parts.len(), 2, "{width}");
        let vs: Vec<f64> = parts.iter().map(|p| p.volume_at(1e-3).unwrap()).collect();
        assert!((vs.iter().sum::<f64>() - v).abs() < 1e-6 * v, "{width}: {vs:?} against {v}; {:?}", d.summary());
        // Above: the plate from 1.5 to 2.5 less the hole; below: less the hole and the groove.
        let (r, h) = (2.5f64, width / 2.0);
        let hole = std::f64::consts::PI * r * r;
        // The strip 20 <= x <= 40, |y - 10| <= h, less the part of it in the hole.
        let groove = width * 20.0 - (h * (r * r - h * h).sqrt() + r * r * (h / r).asin());
        let want = [800.0 - hole, 1.5 * (800.0 - hole - groove)];
        for (got, want) in vs.iter().zip(want) {
            assert!((got - want).abs() < 0.05, "{width}: {vs:?} against {want}; {:?}", d.summary());
        }
        // Both parts end in the plane, not off it.
        let z: Vec<(f64, f64)> = parts.iter().map(|p| crate::measure(p).map(|m| (m.bbox.min.z, m.bbox.max.z)).unwrap()).collect();
        assert!((z[0].0 - 1.5).abs() < 1e-6 && (z[1].1 - 1.5).abs() < 1e-6, "{width}: {z:?}; {:?}", d.summary());
    }
}
