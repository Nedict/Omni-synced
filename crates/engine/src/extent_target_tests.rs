use crate::Session;
use serde_json::json;
use solidcraft_doc::{Document, FeatureKind};

fn model(face: bool) -> Session {
    let mut s = Session::default();
    for (cmd, args) in [
        ("parameters.add", json!({"name":"height","expression":"30 mm","unit":"mm"})),
        ("solid.box", json!({"length":40,"width":30,"height":"height","name":"Base","body_name":"Base"})),
        ("construct.plane.offset", json!({"base":"XY","offset":"height - 10 mm","name":"Stop"})),
        ("sketch.create", json!({"plane":"XY","name":"Profile"})),
        ("sketch.rectangle.two_point", json!({"p0":[5,5],"p1":[15,15]})),
        ("sketch.finish", json!({})),
        (
            "solid.extrude",
            json!({"to_plane":if face {json!({"face":[20,15,30]})} else {json!("Stop")},"to_offset":"-5 mm","operation":"cut","targets":["Base"],"name":"Pocket"}),
        ),
    ] {
        s.execute(cmd, &args).unwrap_or_else(|e| panic!("{cmd}: {e}"));
    }
    s
}

fn volume(s: &mut Session, want: f64) {
    s.refresh();
    assert!(s.model.results.iter().all(|r| r.error.is_none()), "{:?}", s.model.results);
    let measure = s.execute("inspect.measure", &json!({})).unwrap();
    assert_eq!(measure["body_count"], 1);
    assert!((measure["total"]["volume_mm3"].as_f64().unwrap() - want).abs() < 1e-5);
}

#[test]
fn planar_endpoints_follow_large_target_edits_reopen_and_undo() {
    for face in [false, true] {
        let mut s = model(face);
        let original = if face { 33500. } else { 34500. };
        volume(&mut s, original);
        s.execute("parameters.change", &json!({"name":"height","expression":"90 mm"})).unwrap();
        let changed = if face { 99500. } else { 100500. };
        volume(&mut s, changed);
        volume(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), changed);
        s.execute("edit.undo", &json!({})).unwrap();
        volume(&mut s, original);
        let mut doc = (*s.doc).clone();
        doc.find_feature(if face { "Base" } else { "Stop" }).map(|f| f.id).map(|id| doc.feature_mut(id).unwrap().suppressed = true).unwrap();
        let mut lost = Session::new(doc);
        lost.refresh();
        assert!(lost.model.results.iter().any(|r| r.name == "Pocket" && r.error.is_some()));
    }
}

#[test]
fn exact_endpoints_reject_ambiguous_records_and_remap_face_producers() {
    let s = model(true);
    let mut raw: serde_json::Value = serde_json::from_str(&s.doc.to_json()).unwrap();
    let index = raw["features"].as_array().unwrap().iter().position(|v| v["name"] == "Pocket").unwrap();
    raw["features"][index]["extent"]["to"] = json!([20, 15, 30]);
    assert!(Document::from_json(&raw.to_string()).is_err());
    raw["features"][index]["extent"].as_object_mut().unwrap().remove("to");
    raw["features"][index]["extent"]["to_plane"]["anchor"] = serde_json::Value::Null;
    assert!(Document::from_json(&raw.to_string()).is_err());
    let mut copied = s.doc.find_feature("Pocket").unwrap().clone();
    let source = s.doc.find_feature("Base").unwrap().id;
    copied.remap_clone(&[(source, 123)].into(), &Default::default(), &[("Base".into(), "Copy".into())].into(), &Default::default()).unwrap();
    let FeatureKind::Extrude { extent, .. } = copied.kind else { panic!("extrude") };
    let solidcraft_doc::PlaneRef::Face { anchor: Some(anchor), .. } = *extent.to_plane.unwrap() else { panic!("face") };
    assert_eq!(anchor.target.origin_feature, 123);
    assert_eq!(anchor.target.body, "Copy");
}

#[test]
fn unsupported_planar_endpoint_combinations_are_atomic() {
    let mut s = model(false);
    let before = s.doc.clone();
    for params in [
        json!({"to_plane":"XZ", "sketch":"Profile"}),
        json!({"to_plane":"Stop", "sketch":"Profile", "through_all":true}),
        json!({"to_plane":"Stop", "sketch":"Profile", "to":[0,0,20]}),
        json!({"to_plane":"Stop", "sketch":"Profile", "distance2":"5 mm"}),
    ] {
        assert!(s.execute("solid.extrude", &params).is_err(), "{params}");
        assert_eq!(s.doc, before);
    }
}

#[test]
fn endpoint_plane_copy_names_are_mapped_once() {
    let s = model(false);
    let mut copied = s.doc.find_feature("Pocket").unwrap().clone();
    let names = [("Stop".into(), "Stop copy".into()), ("Stop copy".into(), "Stop copy 2".into())].into();
    copied.remap_clone(&Default::default(), &names, &Default::default(), &Default::default()).unwrap();
    let FeatureKind::Extrude { extent, .. } = copied.kind else { panic!("extrude") };
    let solidcraft_doc::PlaneRef::Construction { name } = *extent.to_plane.unwrap() else { panic!("plane") };
    assert_eq!(name, "Stop copy");
}

#[test]
fn logical_caps_follow_signed_height_and_keep_exact_producer_identity() {
    for negative in [false, true] {
        let mut s = Session::default();
        let end_z = if negative { -30. } else { 30. };
        for (cmd, args) in [
            ("parameters.add", json!({"name":"height","expression":"30 mm","unit":"mm"})),
            ("sketch.create", json!({"plane":"XY","name":"Base sketch"})),
            ("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,30]})),
            ("sketch.finish", json!({})),
            ("solid.extrude", json!({"distance":"height","direction":if negative {"negative"} else {"positive"},"name":"Base","body_name":"Base"})),
            ("sketch.create", json!({"plane":"XY","name":"Pocket sketch"})),
            ("sketch.rectangle.two_point", json!({"p0":[5,5],"p1":[15,15]})),
            ("sketch.finish", json!({})),
        ] {
            s.execute(cmd, &args).unwrap();
        }
        // The logical start remains at the source plane while the travel reverses.
        s.execute("solid.extrude", &json!({"to_plane":{"face":[20,15,0],"cap":"start"},"start_offset":if negative {"-height / 6"} else {"height / 6"},"operation":"cut","targets":["Base"]})).unwrap();
        volume(&mut s, 35500.);
        s.execute("parameters.change", &json!({"name":"height","expression":"-40 mm"})).unwrap();
        volume(&mut s, 48000. - 4000. / 6.);
        volume(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), 48000. - 4000. / 6.);
        s.execute("edit.undo", &json!({})).unwrap();
        s.execute("edit.undo", &json!({})).unwrap();
        let before = s.doc.clone();
        for cap in ["start", "invalid"] {
            assert!(s.execute("solid.extrude", &json!({"to_plane":{"face":[20,15,end_z],"cap":cap},"operation":"cut"})).is_err());
            assert_eq!(s.doc, before);
        }
        s.execute(
            "solid.extrude",
            &json!({"to_plane":{"face":[20,15,end_z],"cap":"end"},"to_offset":"-5 mm","operation":"cut","targets":["Base"],"name":"Pocket"}),
        )
        .unwrap();
        volume(&mut s, 33500.);
        s.execute("parameters.change", &json!({"name":"height","expression":"-40 mm"})).unwrap();
        volume(&mut s, 44500.);
        volume(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), 44500.);
        s.execute("edit.undo", &json!({})).unwrap();
        volume(&mut s, 33500.);

        let mut raw: serde_json::Value = serde_json::from_str(&s.doc.to_json()).unwrap();
        let pocket = raw["features"].as_array_mut().unwrap().iter_mut().find(|f| f["name"] == "Pocket").unwrap();
        pocket["extent"]["to_plane"]["anchor"]["target"]["certificate"]["end"] = json!(1);
        assert!(Document::from_json(&raw.to_string()).is_err());
        let mut copy = s.doc.find_feature("Pocket").unwrap().clone();
        let source = s.doc.find_feature("Base").unwrap().id;
        let sketch = s.doc.find_feature("Base sketch").unwrap().id;
        copy.remap_clone(&[(source, 123), (sketch, 124)].into(), &Default::default(), &[("Base".into(), "Copy".into())].into(), &Default::default())
            .unwrap();
        let FeatureKind::Extrude { extent, .. } = copy.kind else { panic!("extrude") };
        let solidcraft_doc::PlaneRef::Face { anchor: Some(anchor), .. } = *extent.to_plane.unwrap() else { panic!("face") };
        assert_eq!(anchor.target.origin_feature, 123);
        assert_eq!(anchor.target.body, "Copy");
        assert!(matches!(anchor.target.certificate, Some(solidcraft_doc::config::FaceCertificate::ExtrudeTravelCap { sketch: 124, end: true, .. })));
        assert!(anchor.target.name.starts_with("F123:cap:end:"));
    }
}
