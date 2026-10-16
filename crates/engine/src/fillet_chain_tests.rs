use super::*;
use serde_json::json;
use solidcraft_doc::FeatureKind;

fn capsule() -> Session {
    let mut s = Session::default();
    s.run_script(&json!([
        {"command":"sketch.create","params":{"plane":"XY"}},
        {"command":"sketch.slot.center_to_center","params":{"p0":[0,0],"p1":[20,0],"width":20}},
        {"command":"sketch.finish","params":{}},
        {"command":"solid.extrude","params":{"distance":40}},
        {"command":"solid.fillet","params":{"edges":[[10,-10,40]],"radius":2,"tangent_chain":true,"name":"Rim"}}
    ]))
    .unwrap();
    s
}

fn check(s: &mut Session, radius: f64, height: f64) {
    assert!(s.model.results.iter().all(|r| r.error.is_none() && r.warning.is_none()), "{:?}", s.model.results);
    let pi = std::f64::consts::PI;
    // Integrate the convex parallel area A - P*s + pi*s^2 through the quarter-circle blend.
    let removed = (40. + 20. * pi) * radius.powi(2) * (1. - pi / 4.) - pi * radius.powi(3) * (5. / 3. - pi / 2.);
    let want = (400. + 100. * pi) * height - removed;
    let actual = s.execute("inspect.measure", &json!({})).unwrap()["total"]["volume_mm3"].as_f64().unwrap();
    assert!((actual - want).abs() < removed * 0.015 + 0.1, "{actual} != {want}");
}

#[test]
fn tangent_seed_history_survives_source_edits_container_reopen_and_undo() {
    let mut s = capsule();
    let rim = s.doc.find_feature("Rim").unwrap();
    let id = rim.id;
    let radius = rim.param_names[0].clone();
    let height = s.doc.find_feature("Extrude1").unwrap().param_names[0].clone();
    assert!(matches!(&rim.kind, FeatureKind::Fillet { tangent_chain: true, edges, .. } if edges.len() == 1));
    assert_eq!(rim.edge_names.len(), 1, "store the seed, not a frozen expanded edge list");
    check(&mut s, 2., 40.);
    let original = s.doc.to_json();
    for (parameter, expression, r, h) in [(&radius, "4 mm", 4., 40.), (&height, "60 mm", 2., 60.)] {
        s.execute("parameters.change", &json!({"name":parameter,"expression":expression})).unwrap();
        check(&mut s, r, h);
        let bytes = io::write_design_container(&s.doc).unwrap();
        let doc = io::read_design(&bytes).unwrap();
        assert_eq!(doc, *s.doc);
        check(&mut Session::new(doc), r, h);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc.to_json(), original);
    }
    s.execute("timeline.edit", &json!({"feature":id,"set":{"tangent_chain":false}})).unwrap();
    assert!(matches!(&s.doc.feature(id).unwrap().kind, FeatureKind::Fillet { tangent_chain: false, .. }));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc.to_json(), original);
}

#[test]
fn fillet_groups_keep_independent_chain_choices_and_reject_invalid_flags() {
    let mut s = Session::default();
    s.execute("solid.box", &json!({"length":20,"width":30,"height":40})).unwrap();
    let original = s.doc.to_json();
    for params in [
        json!({"edges":[[0,0,20]],"radius":2,"tangent_chain":"true"}),
        json!({"edges":[[0,0,20]],"radius":2,"type":"chord","tangent_chain":true}),
        json!({"edge_sets":[{"edges":[[0,0,20]],"radius":2,"tangent_chain":1},{"edges":[[20,30,20]],"radius":3}]}),
    ] {
        assert!(s.execute("solid.fillet", &params).is_err());
        assert_eq!(s.doc.to_json(), original);
    }
    s.execute(
        "solid.fillet",
        &json!({"edge_sets":[
            {"edges":[[0,0,20]],"radius":2,"tangent_chain":true},
            {"edges":[[20,30,20]],"radius":3,"tangent_chain":false}
        ]}),
    )
    .unwrap();
    let FeatureKind::Fillet { tangent_chain, style: doc::FilletStyle::Grouped { groups }, .. } = &s.doc.features.last().unwrap().kind else {
        panic!()
    };
    assert!(*tangent_chain && !groups[0].tangent_chain);
    let doc = io::read_design(&io::write_design_container(&s.doc).unwrap()).unwrap();
    assert_eq!(doc, *s.doc);
}

#[test]
fn tangent_patterns_resolve_the_chain_on_the_target_body() {
    let mut s = Session::default();
    s.run_script(&json!([
        {"command":"sketch.create","params":{"plane":"XY"}},
        {"command":"sketch.slot.center_to_center","params":{"p0":[0,0],"p1":[20,0],"width":20}},
        {"command":"sketch.slot.center_to_center","params":{"p0":[60,0],"p1":[80,0],"width":20}},
        {"command":"sketch.finish","params":{}},
        {"command":"solid.extrude","params":{"distance":40}},
        {"command":"solid.fillet","params":{"edges":[[10,-10,40]],"radius":2,"tangent_chain":true,"name":"Rim"}},
        {"command":"solid.pattern.rectangular","params":{"features":["Rim"],"dir1":[1,0,0],"count1":2,"spacing1":60}}
    ]))
    .unwrap();
    assert!(s.model.results.iter().all(|r| r.error.is_none() && r.warning.is_none()), "{:?}", s.model.results);
    let pi = std::f64::consts::PI;
    let removed = (40. + 20. * pi) * 4. * (1. - pi / 4.) - pi * 8. * (5. / 3. - pi / 2.);
    let want = 2. * ((400. + 100. * pi) * 40. - removed);
    let measure = s.execute("inspect.measure", &json!({})).unwrap();
    assert_eq!(measure["body_count"], 2);
    assert!((measure["total"]["volume_mm3"].as_f64().unwrap() - want).abs() < 2. * removed * 0.015 + 0.2);
    let bytes = io::write_design_container(&s.doc).unwrap();
    let mut reopened = Session::new(io::read_design(&bytes).unwrap());
    assert!(reopened.model.results.iter().all(|r| r.error.is_none() && r.warning.is_none()));
    let measured = reopened.execute("inspect.measure", &json!({})).unwrap();
    assert!((measured["total"]["volume_mm3"].as_f64().unwrap() - want).abs() < 2. * removed * 0.015 + 0.2);
}

#[test]
fn two_radius_groups_cannot_claim_the_same_tangent_chain() {
    let s = capsule();
    let mut doc = (*s.doc).clone();
    doc.features.pop();
    let mut s = Session::new(doc);
    let original = s.doc.to_json();
    let result = s.execute(
        "solid.fillet",
        &json!({"edge_sets":[
            {"edges":[[10,-10,40]],"radius":2,"tangent_chain":true},
            {"edges":[[-10,0,40]],"radius":3,"tangent_chain":true}
        ]}),
    );
    assert!(result.unwrap_err().to_string().contains("more than one fillet selection"));
    assert_eq!(s.doc.to_json(), original);
}
