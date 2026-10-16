use super::*;
use solidcraft_doc::Model;

#[test]
fn cut_caps_follow_the_retained_solid_normal_and_exact_nominal_plane() {
    for top in [false, true] {
        let bytes: &[u8] = if top {
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/solidcraft-cli/tests/history/projected-face-cut-plain-top.solidcraft"))
        } else {
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/solidcraft-cli/tests/history/projected-face-cut-plain-bottom.solidcraft"))
        };
        let mut doc = crate::read_design(bytes).unwrap();
        let creator = doc.find_feature("Block").unwrap().id;
        let cut = doc.find_feature("Cap pocket").unwrap().id;
        doc.features.retain(|feature| feature.id <= cut);
        let FeatureKind::Extrude { targets, .. } = &mut doc.feature_mut(cut).unwrap().kind else { panic!("cut") };
        *targets = vec!["Block".into()];
        let mut model = Model::new();
        for height in [40., 50.] {
            doc.change_param("height", &format!("{height} mm"), None, None).unwrap();
            let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
            model.evaluate(&reopened);
            assert!(model.results.iter().all(|r| r.error.is_none()));
            let state = model.state();
            let anchor = cap_anchor(&reopened, &state, creator, &BTreeMap::from([(cut, true)])).unwrap();
            let plane = reopened.resolve_face_anchor(&anchor, 0, &state).unwrap();
            assert!((plane.origin.z - if top { height - 5. } else { 5. }).abs() < 1e-7);
            assert!(plane.normal().dot(Vec3::Z) * if top { 1. } else { -1. } > 1. - 1e-9);
            assert!(cap_anchor(&reopened, &state, creator, &BTreeMap::from([(cut, false)])).is_err());
            let mut wrong = reopened.clone();
            wrong.feature_mut(cut).unwrap().suppressed = true;
            assert!(cap_anchor(&wrong, &state, creator, &BTreeMap::from([(cut, true)])).is_err());
            wrong.feature_mut(cut).unwrap().suppressed = false;
            let FeatureKind::Extrude { targets, .. } = &mut wrong.feature_mut(cut).unwrap().kind else { panic!("cut") };
            *targets = vec!["Other body".into()];
            assert!(cap_anchor(&wrong, &state, creator, &BTreeMap::from([(cut, true)])).is_err());
            wrong = reopened;
            wrong.change_param("extension", "-8 mm", None, None).unwrap();
            assert!(cap_anchor(&wrong, &state, creator, &BTreeMap::from([(cut, true)])).is_err(), "stale geometry cannot satisfy a changed cut end");
        }
    }
}

#[test]
fn joined_arc_face_projection_follows_edits_and_saved_history() {
    use solidcraft_doc::sketch::{LinkGeom, LinkKind, Sketch};
    let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/solidcraft-cli/tests/history/partial-cylinder-wall-join.solidcraft"));
    let mut doc = crate::read_design(bytes).unwrap();
    let creator = doc.find_feature("Block").unwrap().id;
    let neck = doc.find_feature("Cap extension").unwrap().id;
    let wing = doc.find_feature("Partial wall wing").unwrap().id;
    let mut model = Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let state = model.state();
    let attached = anchor(&doc, &state, creator, &BTreeMap::from([(neck, true), (wing, false)])).unwrap();
    let plane = doc.resolve_face_anchor(&attached, 0, &state).unwrap();
    let resolved = solidcraft_doc::project::resolve_anchored_face(&doc, &state, &plane, LinkKind::Project, &attached).unwrap();
    let geometry = solidcraft_doc::project::join_face_arcs(resolved.geom).unwrap();
    assert_eq!(geometry.len(), 4);
    assert_eq!(geometry.iter().filter(|g| matches!(g, LinkGeom::Arc { .. })).count(), 1);
    let mut sketch = Sketch::new();
    let link = sketch.add_link(LinkKind::Project, resolved.source, &geometry).unwrap();
    let entry = sketch.links.iter_mut().find(|l| l.id == link).unwrap();
    entry.auto_face = true;
    entry.join_arcs = true;
    entry.face_name = Some(attached.target.name.clone());
    let feature = doc
        .add_feature(
            solidcraft_doc::FeatureKind::Sketch {
                plane: solidcraft_doc::PlaneRef::Face {
                    plane,
                    at: plane.origin,
                    name: Some(attached.target.name.clone()),
                    anchor: Some(Box::new(attached)),
                },
                sketch,
            },
            Some("Projected merged cap"),
        )
        .unwrap();
    let kind = serde_json::from_value(serde_json::json!({
        "type":"extrude", "sketch":feature, "profiles":{"type":"all"},
        "extent":{"distance":"1 mm","direction":"positive","convention":"legacy_per_side"},
        "operation":"new_body", "targets":[]
    }))
    .unwrap();
    let copy = doc.add_feature(kind, Some("Cap copy")).unwrap();
    doc.feature_mut(copy).unwrap().body_names = vec!["Cap copy".into()];
    for (diameter, height, depth) in [(20., 40., 10.), (20., 50., 10.), (22., 50., 8.)] {
        doc.change_param("diameter", &format!("{diameter} mm"), None, None).unwrap();
        doc.change_param("height", &format!("{height} mm"), None, None).unwrap();
        doc.change_param("wing_depth", &format!("{depth} mm"), None, None).unwrap();
        let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
        assert_eq!(doc, reopened);
        model.evaluate(&reopened);
        assert!(model.results.iter().all(|r| r.error.is_none()), "{:?}", model.results);
        let state = model.state();
        let projected = state.sketch(feature).unwrap();
        assert_eq!(projected.sketch.curves.len(), 4);
        assert!(projected.sketch.links.iter().all(|l| !l.lost));
        let r = diameter / 2. - 2.;
        let a: f64 = 1.875;
        let wing_area = 2. * a * 25. - a * (r * r - a * a).sqrt() - r * r * (a / r).asin();
        let expected = std::f64::consts::PI * r * r + wing_area;
        let measured = solidcraft_kernel::measure(&state.body("Cap copy").unwrap().body).unwrap().volume;
        assert!((measured - expected).abs() < expected * 1e-3, "cap volume {measured} != {expected}");
        assert!((projected.plane.origin.z - height - 15.).abs() < 1e-7);
    }
}

#[test]
fn mixed_travel_caps_attach_to_one_history_face_and_follow_edits() {
    let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/solidcraft-cli/tests/history/partial-cylinder-wall-join.solidcraft"));
    let mut doc = crate::read_design(bytes).unwrap();
    let creator = doc.find_feature("Block").unwrap().id;
    let neck = doc.find_feature("Cap extension").unwrap().id;
    let wing = doc.find_feature("Partial wall wing").unwrap().id;
    let roles = BTreeMap::from([(neck, true), (wing, false)]);
    let mut model = Model::new();
    model.evaluate(&doc);
    assert!(model.results.iter().all(|r| r.error.is_none()));
    let attached = anchor(&doc, &model.state(), creator, &roles).unwrap();
    let plane = doc.resolve_face_anchor(&attached, 0, &model.state()).unwrap();
    assert!((plane.origin.z - 55.0).abs() < 1e-8);
    assert!(matches!(attached.target.certificate, Some(FaceCertificate::Lineage)));
    for invalid in [BTreeMap::from([(neck, true)]), BTreeMap::from([(neck, false), (wing, false)]), BTreeMap::from([(neck, true), (wing, true)])] {
        assert!(anchor(&doc, &model.state(), creator, &invalid).is_err());
    }
    let mut changed = doc.clone();
    changed.feature_mut(wing).unwrap().suppressed = true;
    assert!(anchor(&changed, &model.state(), creator, &roles).is_err());
    changed.feature_mut(wing).unwrap().suppressed = false;
    let FeatureKind::Extrude { targets, .. } = &mut changed.feature_mut(wing).unwrap().kind else { panic!("wing") };
    *targets = vec!["Foreign body".into()];
    assert!(anchor(&changed, &model.state(), creator, &roles).is_err());

    let mut sketch = solidcraft_doc::sketch::Sketch::new();
    sketch.add_circle(solidcraft_geom::Vec2::ZERO, 2.0, None, None).unwrap();
    let source = doc
        .add_feature(
            FeatureKind::Sketch {
                plane: solidcraft_doc::PlaneRef::Face {
                    plane,
                    at: plane.origin,
                    name: Some(attached.target.name.clone()),
                    anchor: Some(Box::new(attached.clone())),
                },
                sketch,
            },
            Some("Composite cap pocket"),
        )
        .unwrap();
    let kind = serde_json::from_value(serde_json::json!({
        "type":"extrude", "sketch":source,"profiles":{"type":"all"},
        "extent":{"distance":"-0.25 mm","direction":"positive","convention":"legacy_per_side"},
        "operation":"cut", "targets":["Block"]
    }))
    .unwrap();
    doc.add_feature(kind, Some("Pocket")).unwrap();
    let base = serde_json::from_slice::<serde_json::Value>(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/solidcraft-cli/tests/history/partial-cylinder-wall-join.json"
    )))
    .unwrap();
    let expected = base["expected"]["volume_mm3"].as_f64().unwrap() - std::f64::consts::PI;
    super::super::super::import_tests::volume(&doc, expected);
    doc.change_param("extension", "20 mm", None, None).unwrap();
    // Import-time validation must also refuse a document paired with stale geometry.
    assert!(anchor(&doc, &model.state(), creator, &roles).is_err());
    doc.change_param("extension", "15 mm", None, None).unwrap();
    doc.change_param("height", "50 mm", None, None).unwrap();
    let reopened = crate::read_design(&crate::write_design_container(&doc).unwrap()).unwrap();
    super::super::super::import_tests::volume(&reopened, expected + std::f64::consts::PI * 1000.0);
    model.evaluate(&reopened);
    let plane = reopened.resolve_face_anchor(&attached, 0, &model.state()).unwrap();
    assert!((plane.origin.z - 65.0).abs() < 1e-8);
}

#[test]
fn unbounded_recipe_keeps_each_cap_role_and_rejects_other_contributors() {
    use super::super::super::import_tests::Fixture;
    use super::super::super::relationship_tests::{string, u32, u64};
    let make = |labels: &[(&str, u64)]| {
        let mut b = vec![0; 6];
        for n in [1, 3, 1, labels.len() as u32] {
            u32(&mut b, n);
        }
        for (label, operation) in labels {
            string(&mut b, label);
            u32(&mut b, 0);
            u32(&mut b, 1);
            u64(&mut b, *operation);
        }
        u32(&mut b, 0);
        string(&mut b, "face_recipe_data");
        u32(&mut b, 0);
        u32(&mut b, u32::MAX);
        b
    };
    let check = |bytes| {
        let mut fixture = Fixture(BTreeMap::new());
        fixture.put(1, super::super::IDENTITY, 3, bytes, None);
        let design = fixture.design();
        let records = Records::new(&design);
        let classes = design.classes.iter().flat_map(|c| c.objects.iter().map(move |id| (*id, c))).collect();
        super::super::cap_recipe(&records, &classes, 1)
    };
    for labels in [[("2", 9001), ("1", 9002)], [("1", 9001), ("2", 9002)]] {
        let bytes = make(&labels);
        let recipe = check(bytes.clone()).unwrap();
        assert_eq!(recipe.cap_roles, labels.iter().map(|(label, id)| (*id, *label == "2")).collect());
        assert!(!recipe.bounded && recipe.side.is_none());
        for keep in 0..bytes.len() {
            assert!(check(bytes[..keep].to_vec()).is_err());
        }
    }
    for labels in [[("2", 9001), ("3", 9002)], [("2", 9001), ("01", 9002)], [("2", 9001), ("1", 9001)], [("2", 9001), ("1", 0)]] {
        assert!(check(make(&labels)).is_err());
    }
}
