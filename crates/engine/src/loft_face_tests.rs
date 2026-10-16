use crate::{Session, doc};
use serde_json::json;

fn model(operation: &str) -> Session {
    model_in_component(operation, false)
}

fn model_in_component(operation: &str, component: bool) -> Session {
    let mut s = Session::default();
    if component {
        s.execute("component.create", &json!({"name":"Part"})).unwrap();
    }
    for (command, params) in [
        ("parameters.add", json!({"name":"diameter", "expression":"20 mm", "unit":"mm"})),
        ("parameters.add", json!({"name":"base_height", "expression":"10 mm", "unit":"mm"})),
        ("parameters.add", json!({"name":"upper_height", "expression":"40 mm", "unit":"mm"})),
        ("sketch.create", json!({"plane":"XY", "name":"Base"})),
        (
            "sketch.draw.dimensioned",
            json!({"command":"sketch.circle.center", "params":{"center":[0,0],"diameter":20},"dimensions":{"diameter":"diameter"}}),
        ),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"distance":"base_height","taper":"0 deg","name":"Pedestal"})),
        ("construct.plane.offset", json!({"base":"XY", "offset":"upper_height", "name":"Upper plane"})),
        ("sketch.create", json!({"plane":"Upper plane", "name":"Upper"})),
        ("sketch.circle.center", json!({"center":[0,0], "diameter":40})),
        ("sketch.finish", json!({})),
        ("solid.loft", json!({"sections":[{"face":[0,0,10]},{"sketch":"Upper"}],"operation":operation,"targets":["Body1"],"name":"Transition"})),
    ] {
        s.execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
    }
    s
}

fn check(s: &mut Session, volume_pi: f64, bodies: usize, top: f64) {
    assert!(s.model.results.iter().all(|r| r.error.is_none()), "{:?}", s.model.results);
    let measured = s.execute("inspect.measure", &json!({})).unwrap();
    assert_eq!(measured["body_count"], bodies);
    let want = volume_pi * std::f64::consts::PI;
    let got = measured["total"]["volume_mm3"].as_f64().unwrap();
    assert!((got - want).abs() < want * 1e-3, "{got} != {want}");
    let high = measured["bodies"].as_array().unwrap().iter().map(|b| b["bbox"]["max"][2].as_f64().unwrap()).reduce(f64::max).unwrap();
    assert!((high - top).abs() < 1e-5);
}

#[test]
fn certified_loft_face_follows_source_size_height_reopen_and_undo() {
    for (op, bodies) in [("new_body", 2), ("join", 1)] {
        let mut s = model(op);
        let original = s.doc.clone();
        check(&mut s, 8000., bodies, 40.);
        for (parameter, expr, want, top) in [
            ("diameter", "30 mm", 11500., 40.),
            ("base_height", "15 mm", 11083.333333333333, 40.),
            ("upper_height", "50 mm", 14166.666666666667, 50.),
        ] {
            s.execute("parameters.change", &json!({"name":parameter,"expression":expr})).unwrap();
            check(&mut s, want, bodies, top);
            let reopened = doc::Document::from_json(&s.doc.to_json()).unwrap();
            assert_eq!(reopened, *s.doc);
            check(&mut Session::new(reopened), want, bodies, top);
        }
        for _ in 0..3 {
            s.execute("edit.undo", &json!({})).unwrap();
        }
        assert_eq!(s.doc, original);
        check(&mut s, 8000., bodies, 40.);
    }
}

#[test]
fn lost_loft_face_does_not_bind_to_reused_body_or_ambiguous_sections() {
    let s = model("new_body");
    let mut document = (*s.doc).clone();
    let feature = document.find_feature("Transition").unwrap().id;
    let doc::FeatureKind::Loft { sections, .. } = &mut document.feature_mut(feature).unwrap().kind else { panic!("loft") };
    sections[0].face.as_mut().unwrap().target.origin_feature = 999;
    let broken = Session::new(document);
    assert!(broken.model.results.iter().any(|r| r.error.is_some()));
    let mut value: serde_json::Value = serde_json::from_str(&s.doc.to_json()).unwrap();
    let loft = value["features"].as_array_mut().unwrap().iter_mut().find(|f| f["name"] == "Transition").unwrap();
    loft["sections"][0]["sketch"] = json!(1);
    assert!(doc::Document::from_json(&value.to_string()).is_err());
}

#[test]
fn loft_face_certificate_is_revoked_when_parent_becomes_tapered() {
    let mut s = model("new_body");
    let feature = s.doc.find_feature("Pedestal").unwrap().id;
    let mut document = (*s.doc).clone();
    let doc::FeatureKind::Extrude { extent, .. } = &mut document.feature_mut(feature).unwrap().kind else { panic!("extrude") };
    extent.taper = Some("5 deg".into());
    let changed = Session::new(document);
    assert!(changed.model.results.iter().any(|r| r.error.is_some()));
    check(&mut s, 8000., 2, 40.);
}

#[test]
fn component_copy_remaps_loft_face_to_its_copied_producer() {
    let mut s = model_in_component("new_body", true);
    let source = s.doc.find_feature("Transition").unwrap().clone();
    let copied = s.execute("component.paste_new", &json!({"component":source.component,"translate":[60,0,0]})).unwrap();
    let component = copied["component"].as_u64().unwrap();
    let loft = s.doc.features.iter().find(|f| f.component == component && matches!(f.kind, doc::FeatureKind::Loft { .. })).unwrap();
    let doc::FeatureKind::Loft { sections, .. } = &loft.kind else { panic!("loft") };
    let face = sections[0].face.as_ref().unwrap();
    assert_eq!(face.component, component);
    let producer = s.doc.feature(face.target.origin_feature).unwrap();
    assert_eq!(producer.component, component);
    assert_eq!(producer.body_names, std::slice::from_ref(&face.target.body));
    assert!(s.model.results.iter().all(|r| r.error.is_none()), "{:?}", s.model.results);
    let expected = 8000. * std::f64::consts::PI;
    let got = s.execute("component.properties", &json!({"component":component})).unwrap()["total"]["volume_mm3"].as_f64().unwrap();
    assert!((got - expected).abs() < expected * 1e-3, "{got} != {expected}");
    let mut reopened = Session::new(doc::Document::from_json(&s.doc.to_json()).unwrap());
    let got = reopened.execute("component.properties", &json!({"component":component})).unwrap()["total"]["volume_mm3"].as_f64().unwrap();
    assert!((got - expected).abs() < expected * 1e-3);
}
