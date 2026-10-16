use super::*;
use crate::f3d::import_tests::{Fixture, volume};
use crate::f3d::relationship_tests::{reference, string, u32, u64};

fn identity(labels: &[&str], operations: &[u64], kind: &str, tree: &[i32]) -> Vec<u8> {
    let mut b = vec![0; 6];
    for n in [1, 3, labels.len() as u32] {
        u32(&mut b, n);
    }
    for (label, operation) in labels.iter().zip(operations) {
        u32(&mut b, 1);
        string(&mut b, label);
        u32(&mut b, 0);
        u32(&mut b, 1);
        u64(&mut b, *operation);
    }
    u32(&mut b, 0);
    string(&mut b, kind);
    for n in tree {
        u32(&mut b, *n as u32);
    }
    b
}

fn fixture() -> Fixture {
    let mut f = super::super::body_projections::tests::body_circle(false);
    f.0.get_mut(&185).unwrap().2 = identity(
        &["1", "2"],
        &[8001, 8999],
        "bounded_face_recipe_data",
        &[0, -1, 1, -1, -1, 2, 0, -1, 1, -1, 1, -1, 2, 1, -1, 0, -1, 0, 1, 0, 2, 1, 1, 1, 1, 0, 1, -1],
    );
    let edge = identity(
        &["-102", "3", "1"],
        &[8001; 3],
        "edge_recipe_data",
        &[0, -1, 2, 1, -1, 2, -1, 1, -1, 2, 1, -1, 2, -1, 1, 0, -1, 0, 0, -1, 1, 0, 2, 1, 1, 1, 1, 0, 1, -1],
    );
    f.0.get_mut(&188).unwrap().2 = edge.clone();
    f.put(288, IDENTITY, 3, edge, None);
    let mut tracker = vec![0; 6];
    reference(&mut tracker, 288);
    tracker.push(0);
    f.put(284, TRACKER, 1, tracker, None);
    f.put(287, "C46D3EEB-40E1-43FA-83DC-9CB204335417", 1, vec![], None);
    let mut circle = vec![0; 6];
    for value in [0_f64, 0., 0., 0., 0., 1., 1., 0., 0., 3., 0., std::f64::consts::TAU] {
        circle.extend(value.to_le_bytes());
    }
    circle.push(0);
    reference(&mut circle, 287);
    circle.extend([0, 1]);
    reference(&mut circle, 284);
    f.0.get_mut(&187).unwrap().2 = circle;
    let map = &mut f.0.get_mut(&280).unwrap().2;
    let at = map.len() - 15;
    let mut retired = Vec::new();
    u32(&mut retired, 1);
    u64(&mut retired, 8999);
    map.splice(at..at + 4, retired);
    f
}

fn decode_fixture(f: Fixture) -> Result<Vec<super::super::projections::Projection>> {
    let d = f.design();
    let records = Records::new(&d);
    let classes = d.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
    decode(&records, &classes, 183, 18, &[(184, 131)])
}

#[test]
fn retired_cap_keeps_a_live_circle_dependency_and_downstream_extrusion() {
    use solidcraft_doc::{FeatureKind, Model};
    let mut doc = fixture().design().to_document("Original retired cap").unwrap();
    let mut kind = doc.features[1].kind.clone();
    let FeatureKind::Extrude { sketch, profiles, extent, .. } = &mut kind else { panic!("extrusion") };
    *sketch = (3).into();
    *profiles = solidcraft_doc::ProfileSel::WholeBoundaries {
        profiles: vec![solidcraft_doc::WholeProfileBoundary { outer: vec!["c131".into()], holes: vec![] }],
    };
    extent.distance = "7 mm".into();
    doc.add_feature(kind, Some("Projected extrusion")).unwrap();
    let verify = |doc: &solidcraft_doc::Document, radius: f64| {
        let mut model = Model::new();
        model.evaluate(doc);
        assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results);
        let state = model.state();
        assert_eq!(state.bodies.len(), 2);
        let projected = state.sketch(3).unwrap();
        let link = &projected.sketch.links[0];
        assert!(!link.lost && !link.edge_circle.is_empty());
        assert!(link.edge_circle.iter().all(|name| name.contains("F2:start")));
        assert!(matches!(projected.sketch.curves[0].kind, solidcraft_doc::sketch::CurveKind::Circle { r, .. } if (r-radius).abs()<1e-6));
        volume(doc, std::f64::consts::PI * radius * radius * 24.);
    };
    verify(&doc, 30.);
    doc.change_param("d7", "40 mm", None, None).unwrap();
    verify(&doc, 20.);
    let mut reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
    assert_eq!(doc, reopened);
    verify(&reopened, 20.);
    reopened.change_param("d7", "80 mm", None, None).unwrap();
    verify(&reopened, 40.);
}

#[test]
fn retired_cap_requires_complete_matching_current_tracking_and_retirement() {
    assert!(decode_fixture(fixture()).is_ok());
    for id in [183, 184, 185, 186, 187, 188, 280, 284, 288] {
        let original = fixture().0[&id].2.clone();
        for keep in 0..original.len() {
            let mut f = fixture();
            f.0.get_mut(&id).unwrap().2.truncate(keep);
            assert!(decode_fixture(f).is_err(), "record {id}, prefix {keep}");
        }
    }
    // Every incidence-tree slot, including framing and tracking sentinels.
    for (id, slots) in [(185, 28), (188, 30), (288, 30)] {
        let n = fixture().0[&id].2.len();
        for slot in 0..slots {
            let mut f = fixture();
            f.0.get_mut(&id).unwrap().2[n - slots * 4 + slot * 4] ^= 0x40;
            assert!(decode_fixture(f).is_err(), "record {id}, tree slot {slot}");
        }
    }
    // An absent operation is insufficient: it must be explicitly retired.
    let mut f = fixture();
    let map = &mut f.0.get_mut(&280).unwrap().2;
    let n = map.len();
    map[n - 19..n - 11].copy_from_slice(&9000_u64.to_le_bytes());
    assert!(decode_fixture(f).is_err());
    // A still-live boundary cannot be silently discarded.
    let mut f = fixture();
    let map = &mut f.0.get_mut(&280).unwrap().2;
    // An empty forward index leaves the reverse table authoritative.
    map.drain(10..30);
    map[6..10].copy_from_slice(&0_u32.to_le_bytes());
    map[10..14].copy_from_slice(&2_u32.to_le_bytes());
    let mut live = Vec::new();
    u64(&mut live, 8999);
    u64(&mut live, 9);
    map.splice(30..30, live);
    assert!(decode_fixture(f).is_err());
    // Different producer tags on either current tracker must not match by shape.
    for id in [188, 288] {
        let mut f = fixture();
        let record = &mut f.0.get_mut(&id).unwrap().2;
        let mut r = Reader { bytes: record, at: 22 };
        r.string().unwrap();
        r.take(8).unwrap();
        let at = r.at;
        record[at..at + 8].copy_from_slice(&8002_u64.to_le_bytes());
        assert!(decode_fixture(f).is_err());
    }
    for slot in 0..12 {
        let mut f = fixture();
        let at = 6 + slot * 8;
        f.0.get_mut(&187).unwrap().2[at..at + 8].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode_fixture(f).is_err(), "nonfinite circle slot {slot}");
    }
}

#[test]
fn retired_cap_never_substitutes_geometry_for_a_mismatching_projected_circle() {
    let mut f = fixture();
    let circle = &mut f.0.get_mut(&131).unwrap().2;
    let mut r = Reader { bytes: circle, at: 6 };
    for _ in 0..r.u32().unwrap() {
        r.string().unwrap();
        r.string().unwrap();
        r.u64().unwrap();
    }
    let at = r.at + 9 * 8;
    circle[at..at + 8].copy_from_slice(&4_f64.to_le_bytes());
    assert!(f.design().to_document("Wrong projected radius").unwrap_err().to_string().contains("disagrees with its source body"));
}

#[test]
#[ignore = "requires the local Style 5 archive"]
fn native_style5_retired_cap_retains_sketch_and_extrusion() {
    let root = std::path::PathBuf::from(solidcraft_geom::env::var_os("F3D_ADDITIONAL_CORPUS").unwrap());
    let bytes = std::fs::read(root.join("knobs/Style 5.f3d")).unwrap();
    let outcome = import_f3d_with_preference(&bytes, "Style 5", ImportPreference::Editability).unwrap();
    for feature in &outcome.document.features {
        println!("FEATURE {} {}", feature.id, feature.name);
    }
    for warning in &outcome.warnings {
        println!("WARNING {warning}");
    }
    assert!(outcome.complete_history && outcome.warnings.is_empty(), "Style 5 must import without recovery");
    assert_eq!(outcome.document.features.len(), 14);
    assert!(outcome.document.find_feature("Sketch2").is_some());
    assert!(outcome.document.find_feature("Extrude2").is_some());
    let reopened = crate::read_design(&crate::write_design_container(&outcome.document).unwrap()).unwrap();
    assert_eq!(reopened, outcome.document);
    let mut model = solidcraft_doc::Model::new();
    model.evaluate(&reopened);
    assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results);
    let state = model.state();
    assert_eq!(state.bodies.len(), 1);
    let volume = solidcraft_kernel::measure(&state.bodies[0].body).unwrap().volume;
    assert!(volume.is_finite() && volume > 0.);
    println!("Complete Style 5 history: {} features, volume {volume:.6} mm3", reopened.features.len());
    // The final feature is a 10 x 10 mm square pocket, 7 mm deep. Its
    // removal is analytic and independent of the native-body fallback mesh.
    let mut uncut = reopened.clone();
    let cut = uncut.find_feature("Extrude2").unwrap().id;
    uncut.features.iter_mut().find(|feature| feature.id == cut).unwrap().suppressed = true;
    model.evaluate(&uncut);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let stock = solidcraft_kernel::measure(&model.state().bodies[0].body).unwrap().volume;
    assert!((stock - volume - 700.).abs() < 0.01, "square pocket removal: {}", stock - volume);
    let mut edited = reopened;
    edited.change_param("InsertDepth", "5 mm", None, None).unwrap();
    model.evaluate(&edited);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let shallower = solidcraft_kernel::measure(&model.state().bodies[0].body).unwrap().volume;
    assert!((shallower - volume - 200.).abs() < 0.01, "square pocket depth edit: {}", shallower - volume);
}
