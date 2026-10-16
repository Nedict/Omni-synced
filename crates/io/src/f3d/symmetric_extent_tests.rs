use super::import_tests::{Fixture, fixture, volume};
use super::relationship_tests::{reference, u32};
use solidcraft_doc::{Direction, Document, FeatureKind, Model};
use std::f64::consts::PI;

// Original records: one driving distance, shared draft, and an explicit
// full-length/per-side discriminator. No exported bytes are bundled.
fn symmetric(full: bool, legacy: bool) -> Fixture {
    let mut f = fixture(false);
    let old = f.0[&9].2.clone();
    let mut b = old[..55].to_vec();
    b[17..21].copy_from_slice(&3_u32.to_le_bytes());
    b[21..25].copy_from_slice(&(if legacy { 2_u32 } else { 1 }).to_le_bytes());
    b.push(0);
    if legacy {
        reference(&mut b, 64);
    } else {
        b.push(0);
    }
    b.extend_from_slice(&old[57..96]);
    b.extend(vec![0; if legacy { 9 } else { 5 }]);
    u32(&mut b, 1);
    b.extend(vec![0; if legacy { 6 } else { 10 }]);
    reference(&mut b, 66);
    b.push(0);
    u32(&mut b, if full { 1 } else { 2 });
    b.extend([0; 11]);
    b.extend_from_slice(&old[132..]);
    f.0.get_mut(&9).unwrap().2 = b;
    f
}

fn centered(doc: &Document, radius: f64, half: f64) {
    volume(doc, PI * radius * radius * half * 2.);
    let mut model = Model::new();
    model.evaluate(doc);
    assert_eq!(model.state().bodies.len(), 1);
    let bounds = model.state().bodies[0].mesh().bounds();
    assert!((bounds.min.z + half).abs() < 1e-6, "{bounds:?}");
    assert!((bounds.max.z - half).abs() < 1e-6, "{bounds:?}");
    let reopened = crate::read_design(&crate::write_design_container(doc).unwrap()).unwrap();
    assert_eq!(*doc, reopened);
    volume(&reopened, PI * radius * radius * half * 2.);
}

#[test]
fn symmetric_lengths_preserve_parameters_centering_and_reopening() {
    for legacy in [false, true] {
        for full in [false, true] {
            let scale = if full { 0.5 } else { 1. };
            let mut doc = symmetric(full, legacy).design().to_document("Symmetric cylinder").unwrap();
            let FeatureKind::Extrude { extent, .. } = &doc.features[1].kind else { panic!("extrude") };
            assert_eq!(extent.direction, Direction::Symmetric);
            assert_eq!(
                extent.convention,
                if full { solidcraft_doc::ExtrudeConvention::TotalDistance } else { solidcraft_doc::ExtrudeConvention::LegacyPerSide }
            );
            assert_eq!(extent.distance, "17 mm");
            assert_eq!(doc.features[1].param_names.first().map(String::as_str), Some("height"));
            assert!(extent.start_offset.is_none());
            assert!(extent.distance2.is_none());
            centered(&doc, 30., 17. * scale);
            // The native named distance remains the same value in both modes;
            // downstream dependencies must never silently receive half of it.
            doc.change_param("d7", "height * 2", None, None).unwrap();
            doc.change_param("height", "23 mm", None, None).unwrap();
            centered(&doc, 23., 23. * scale);
            doc.change_param("height", "-11 mm", None, None).unwrap();
            doc.change_param("d7", "40 mm", None, None).unwrap();
            centered(&doc, 20., 11. * scale);
        }
    }
}

#[test]
fn symmetric_extent_rejects_inconsistent_bindings_flags_and_truncation() {
    for legacy in [false, true] {
        let base = symmetric(false, legacy);
        let shift = if legacy { 10 } else { 0 };
        // Corrupt direction, unsupported support, first extent, symmetric
        // marker, repeated taper, length mode and trailing reserved fields.
        for (at, value) in
            [(17, 2), (27, 1), (92 + shift, 2), (if legacy { 115 } else { 101 }, 2), (116 + shift, 64), (127 + shift, 3), (141 + shift, 1)]
        {
            let mut f = Fixture(base.0.clone());
            f.0.get_mut(&9).unwrap().2[at] = value;
            assert!(f.design().history().is_err(), "legacy={legacy}, byte={at}");
        }
        if legacy {
            let mut f = Fixture(base.0.clone());
            f.0.get_mut(&9).unwrap().2[57] = 66;
            assert!(f.design().history().is_err());
        }
        for keep in 0..142 + shift {
            let mut f = Fixture(base.0.clone());
            f.0.get_mut(&9).unwrap().2.truncate(keep);
            assert!(f.design().history().is_err(), "legacy={legacy}, prefix={keep}");
        }
    }
}

#[test]
#[ignore = "requires local controlled symmetric-extent archives"]
fn native_symmetric_extents_keep_both_distance_conventions_editable() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_PROBES").unwrap());
    for full in [false, true] {
        for distance in [30, 40] {
            let bytes = std::fs::read(root.join(format!("symmetric-{}-{distance}-0.f3d", u8::from(full)))).unwrap();
            let mut doc = super::import_f3d(&bytes, "Symmetric").unwrap();
            assert_eq!(doc.features.len(), 2);
            let scale = if full { 0.5 } else { 1. };
            centered(&doc, 50., f64::from(distance) * scale);
            doc.change_param("d2", "25 mm", None, None).unwrap();
            doc.change_param("d1", "d2 * 2", None, None).unwrap();
            centered(&doc, 25., 25. * scale);
        }
    }
}
