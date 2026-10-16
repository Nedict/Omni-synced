use super::*;
use serde_json::json;
use solidcraft_doc::FeatureKind;

fn block() -> Session {
    let mut s = Session::default();
    s.execute("solid.box", &json!({"length":20,"width":30,"height":40})).unwrap();
    s
}

fn fillet(s: &mut Session) -> u64 {
    s.execute(
        "solid.fillet",
        &json!({"name":"Rounds","edge_sets":[
            {"edges":[[0,0,20]],"radius":"2 mm"},
            {"edges":[[20,30,20]],"radius":"3 mm"}
        ]}),
    )
    .unwrap()["feature"]
        .as_u64()
        .unwrap()
}

fn check(s: &mut Session, radii: [f64; 2], height: f64, copies: f64) {
    assert!(s.model.results.iter().all(|r| r.error.is_none() && r.warning.is_none()), "{:?}", s.model.results);
    let removed = (1. - std::f64::consts::PI / 4.) * radii.iter().map(|r| r * r).sum::<f64>() * height * copies;
    let expected = 20. * 30. * height * copies - removed;
    let measured = s.execute("inspect.measure", &json!({})).unwrap()["total"]["volume_mm3"].as_f64().unwrap();
    assert!((measured - expected).abs() < removed * 0.005 + 0.01, "{measured} != {expected}");
}

#[test]
fn grouped_fillet_radii_keep_parameters_reopen_source_edits_and_undo() {
    let mut s = block();
    let id = fillet(&mut s);
    let names = s.doc.feature(id).unwrap().param_names.clone();
    assert_eq!(names.len(), 2);
    assert_ne!(names[0], names[1]);
    assert_eq!(s.doc.feature(id).unwrap().edge_names.len(), 2);
    check(&mut s, [2., 3.], 40., 1.);
    let before = s.doc.to_json();
    for (name, value, expected) in [(&names[0], "4 mm", [4., 3.]), (&names[1], "5 mm", [2., 5.])] {
        s.execute("parameters.change", &json!({"name":name,"expression":value})).unwrap();
        check(&mut s, expected, 40., 1.);
        let doc = Document::from_json(&s.doc.to_json()).unwrap();
        check(&mut Session::new(doc), expected, 40., 1.);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc.to_json(), before);
    }
    s.execute("timeline.edit", &json!({"feature":1,"set":{"height":"50 mm"}})).unwrap();
    check(&mut s, [2., 3.], 50., 1.);
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("timeline.suppress", &json!({"feature":id,"suppressed":true})).unwrap();
    check(&mut s, [0., 0.], 40., 1.);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc.to_json(), before);
}

#[test]
fn grouped_fillet_redefinition_and_patterns_keep_each_radius() {
    let mut s = block();
    s.execute("solid.box", &json!({"length":20,"width":30,"height":40,"corner":[40,0,0]})).unwrap();
    let id = fillet(&mut s);
    let names = s.doc.feature(id).unwrap().param_names.clone();
    s.execute(
        "timeline.redefine",
        &json!({"feature":id,"command":"solid.fillet","params":{"edge_sets":[
            {"edges":[[0,0,20]],"radius":"2 mm"}, {"edges":[[20,30,20]],"radius":"5 mm"}
        ]}}),
    )
    .unwrap();
    assert_eq!(s.doc.feature(id).unwrap().param_names, names);
    s.execute("solid.pattern.rectangular", &json!({"features":["Rounds"],"dir1":[1,0,0],"count1":2,"spacing1":40})).unwrap();
    check(&mut s, [2., 5.], 40., 2.);
    s.execute("parameters.change", &json!({"name":names[1],"expression":"3 mm"})).unwrap();
    check(&mut s, [2., 3.], 40., 2.);
    check(&mut Session::new(Document::from_json(&s.doc.to_json()).unwrap()), [2., 3.], 40., 2.);
}

#[test]
fn invalid_fillet_groups_fail_atomically_instead_of_reassigning_radii() {
    let mut s = block();
    let original = s.doc.to_json();
    for p in [
        json!({"edge_sets":[]}),
        json!({"edge_sets":[{"edges":[[0,0,20]],"radius":2}]}),
        json!({"edge_sets":[{"edges":[[0,0,20]],"radius":2},{"edges":[[0,0,10]],"radius":3}]}),
        json!({"edge_sets":[{"edges":[[0,0,20]],"radius":2},{"edges":[],"radius":3}]}),
        json!({"fit":"reduce","edge_sets":[{"edges":[[0,0,20]],"radius":2},{"edges":[[20,30,20]],"radius":3}]}),
        json!({"radius":2,"edge_sets":[{"edges":[[0,0,20]],"radius":2},{"edges":[[20,30,20]],"radius":3}]}),
        json!({"corner":"g2","edge_sets":[{"edges":[[0,0,20]],"radius":2},{"edges":[[20,30,20]],"radius":3}]}),
    ] {
        assert!(s.execute("solid.fillet", &p).is_err(), "{p}");
        assert_eq!(s.doc.to_json(), original);
    }
    let id = fillet(&mut s);
    for count in [0, 2, usize::MAX] {
        let mut doc = (*s.doc).clone();
        let FeatureKind::Fillet { style: solidcraft_doc::FilletStyle::Grouped { groups }, .. } = &mut doc.feature_mut(id).unwrap().kind else {
            panic!()
        };
        groups[0].count = count;
        let reloaded = Document::from_json(&doc.to_json()).unwrap();
        let model = Session::new(reloaded);
        assert!(model.model.result(id).unwrap().error.is_some());
    }
}
