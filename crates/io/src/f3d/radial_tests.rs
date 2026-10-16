use super::import_tests::{Fixture, fixture, volume};
use super::relationship_tests::{reference, refs, u32, u64, wide};
use super::round_profile_tests::{arc, geometry};
use solidcraft_doc::{FeatureKind, sketch::ConstraintKind};
use std::f64::consts::PI;

// Original field-authored records, independently constructed from the observed
// schemas. No exported native byte sequences are included in the test suite.
fn dimensioned(is_arc: bool, radial: bool, unit: &str) -> Fixture {
    let mut f = if is_arc { arc(PI, 0.) } else { fixture(false) };
    if is_arc {
        let base = fixture(false);
        for id in [8, 56, 61, 62] {
            f.0.insert(id, base.0[&id].clone());
        }
        geometry(&mut f, &[42, 41, 71, 72, 31, 35, 56], &[56]);
    }
    let value: f64 = if radial { 3. } else { 6. };
    let expression = format!("{} {unit}", value * 10.);
    let mut b = vec![0; 4];
    b.push(1);
    u32(&mut b, 2);
    reference(&mut b, 0);
    u32(&mut b, 4);
    reference(&mut b, 31);
    u32(&mut b, 0);
    b.push(1);
    u32(&mut b, 0);
    b.extend([0; 244]);
    wide(&mut b, if radial { "fx: R" } else { "fx: Ø" });
    wide(&mut b, &expression);
    b.extend([0; 24]);
    u64(&mut b, if radial { 0x20000 } else { 0x10000 });
    reference(&mut b, 61);
    refs(&mut b, &[31]);
    b.push(0);
    let r = f.0.get_mut(&56).unwrap();
    r.0 = if radial { "8EDAC24D-35D4-4591-B8E7-B3EA081C9DF0" } else { "664AD49F-207F-4CD5-8555-DD269E9F09EA" }.into();
    r.2 = b;
    let mut b = vec![0; 20];
    u32(&mut b, 7);
    reference(&mut b, 61);
    wide(&mut b, &expression);
    b.extend([0; 10]);
    for s in [if radial { "Radius" } else { "Diameter" }, "", unit, "d7"] {
        wide(&mut b, s);
    }
    b.extend(value.to_le_bytes());
    b.extend([0; 12]);
    f.0.get_mut(&62).unwrap().2 = b;
    let mut b = vec![0; 4];
    b.push(1);
    refs(&mut b, &[8]);
    b.extend([0; 5]);
    b.extend(value.to_le_bytes());
    reference(&mut b, 62);
    f.0.get_mut(&61).unwrap().2 = b;
    f
}

#[test]
fn radial_and_diameter_bindings_preserve_circle_and_arc_edits() {
    for is_arc in [false, true] {
        for radial in [false, true] {
            let mut doc = dimensioned(is_arc, radial, "mm").design().to_document("Original dimensioned round profile").unwrap();
            let FeatureKind::Sketch { sketch, .. } = &doc.features[0].kind else { panic!("sketch") };
            let dim = sketch.constraints.iter().find(|c| c.param.as_deref() == Some("d7")).unwrap();
            assert_eq!(matches!(dim.kind, ConstraintKind::Radius { .. }), radial);
            assert!(!dim.driven);
            let factor = if is_arc { 0.5 } else { 1. };
            volume(&doc, 900. * PI * 17. * factor);
            doc.change_param("d7", if radial { "40 mm" } else { "80 mm" }, None, None).unwrap();
            volume(&doc, 1600. * PI * 17. * factor);
            let reopened = crate::read_design(&crate::write_design(&doc)).unwrap();
            assert_eq!(doc, reopened);
            volume(&reopened, 1600. * PI * 17. * factor);
        }
    }
}

#[test]
fn radial_schema_rejects_truncation_wrong_flags_and_units() {
    let f = dimensioned(true, true, "mm");
    let len = f.0[&56].2.len();
    for keep in 0..len {
        let mut f = dimensioned(true, true, "mm");
        f.0.get_mut(&56).unwrap().2.truncate(keep);
        assert!(f.design().to_document("Truncated radius").is_err(), "{keep}");
    }
    let mut f = dimensioned(true, true, "mm");
    f.0.get_mut(&56).unwrap().0 = "664AD49F-207F-4CD5-8555-DD269E9F09EA".into();
    assert!(f.design().sketch_constraints().unwrap_err().to_string().contains("flags disagree"));
    for unit in ["deg", ""] {
        assert!(dimensioned(false, true, unit).design().to_document("Invalid units").unwrap_err().to_string().contains("length parameter"));
    }
}
