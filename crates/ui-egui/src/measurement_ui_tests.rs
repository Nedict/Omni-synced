//! Circle and arc centres are picked through the viewport; the displayed distance remains
//! visible until the measurement dialog closes.
use crate::dialogs::Kind;
use crate::scenario::Harness;
use serde_json::{Value, json};
use solidcraft_engine::Sel;

fn result(h: &Harness) -> &Value {
    match &h.app.dialog.as_ref().unwrap().kind {
        Kind::Measure { result: Some(r), .. } => r,
        _ => panic!("expected a measurement result"),
    }
}

fn scene(command: &str, params: Value, finished: bool) -> Harness {
    let mut h = Harness::new();
    h.app.ui.show_origin = false;
    h.app.ui.show_browser = false;
    h.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
    h.app.run(command, params).unwrap();
    h.app.run("sketch.line", json!({"points": [[0, 0], [0, 30]]})).unwrap();
    if finished {
        h.app.run("sketch.finish", json!({})).unwrap();
    }
    h.step(&json!({"call": "ui.view", "params": {"view": "top"}})).unwrap();
    h.app.start("inspect.measure");
    h.step(&json!({"call": "ui.view", "params": {"view": "fit"}})).unwrap();
    h.settle();
    h
}

fn click_world(h: &mut Harness, p: [f64; 3]) {
    let at = h.at(&json!({"world": p})).unwrap();
    h.step(&json!({"click": at})).unwrap();
    h.settle();
}

fn cylinder_scene(top: bool) -> Harness {
    let mut h = Harness::new();
    h.app.ui.show_origin = false;
    h.app.ui.show_browser = false;
    h.app.run("solid.cylinder", json!({"radius":4,"height":12,"body_name":"A"})).unwrap();
    h.app.run("solid.cylinder", json!({"base":[20,0,7],"radius":4,"height":12,"body_name":"B","operation":"new_body"})).unwrap();
    h.step(&json!({"call":"ui.view","params":{"view":if top {"top"}else{"iso"}}})).unwrap();
    h.app.start("inspect.measure");
    h.step(&json!({"call":"ui.view","params":{"view":"fit"}})).unwrap();
    h.settle();
    h
}

fn use_center(h: &mut Harness, item: usize) {
    let at = crate::scenario::handle_at(&format!("measure:center:{item}")).unwrap();
    h.step(&json!({"click":[at.x,at.y]})).unwrap();
    h.settle();
}

#[test]
fn measurement_3d_cylinder_faces_can_switch_independently_to_their_center_axes() {
    let mut h = cylinder_scene(false);
    click_world(&mut h, [0., -4., 6.]);
    click_world(&mut h, [20., -4., 13.]);
    let r = result(&h);
    assert_eq!(r["items"][0]["type"], "face", "{r}");
    assert_eq!(r["items"][1]["type"], "face", "{r}");
    assert!(r["distance_mm"].as_f64().unwrap() < 12.1, "{r}");
    use_center(&mut h, 1);
    assert_eq!(result(&h)["items"][0]["reference"], "axis");
    assert!(result(&h)["items"][1].get("reference").is_none());
    use_center(&mut h, 2);
    let r = result(&h);
    assert!((r["distance_mm"].as_f64().unwrap() - 20.).abs() < 1e-8, "{r}");
    assert!(crate::scenario::handle_at("measure:label").is_some());
    use_center(&mut h, 1);
    use_center(&mut h, 2);
    assert!(result(&h)["distance_mm"].as_f64().unwrap() < 12.1);
}

#[test]
fn measurement_model_rims_use_centres_and_keep_the_reference_attached_to_its_item() {
    let mut h = cylinder_scene(true);
    let a = [2., 12.0_f64.sqrt(), 12.];
    let b = [22., 12.0_f64.sqrt(), 19.];
    click_world(&mut h, a);
    click_world(&mut h, b);
    assert!(h.app.dialog.as_ref().unwrap().items().iter().all(|s| matches!(s, Sel::Edge { .. })), "{:?}", h.app.dialog.as_ref().unwrap().items());
    use_center(&mut h, 1);
    use_center(&mut h, 2);
    let r = result(&h);
    assert!((r["distance_mm"].as_f64().unwrap() - 449.0_f64.sqrt()).abs() < 1e-8, "{r}");
    click_world(&mut h, a);
    assert_eq!(result(&h)["items"][0]["reference"], "center");
    click_world(&mut h, a);
    let r = result(&h);
    assert_eq!(r["items"][0]["reference"], "center");
    assert!(r["items"][1].get("reference").is_none(), "new picks use their own reference: {r}");
}

#[test]
fn measurement_clicks_circle_and_arc_centres_in_active_and_finished_sketches() {
    for finished in [false, true] {
        for (cmd, params) in [
            ("sketch.circle.center", json!({"center": [12.3, 9.7], "radius": 4})),
            ("sketch.arc.center_point", json!({"center": [12.3, 9.7], "start": [16.3, 9.7], "end": [12.3, 13.7]})),
            ("sketch.arc.three_point", json!({"start": [16.3, 9.7], "end": [8.3, 9.7], "through": [12.3, 13.7]})),
        ] {
            let mut h = scene(cmd, params, finished);
            click_world(&mut h, [12.3, 9.7, 0.0]);
            assert!(matches!(h.app.dialog.as_ref().unwrap().items().as_slice(), [Sel::ExternalSketchPoint { .. }]), "{cmd}, finished={finished}");
            click_world(&mut h, [0.0, 9.7, 0.0]);
            let r = result(&h);
            assert!((r["distance_mm"].as_f64().unwrap() - 12.3).abs() < 1e-8, "{cmd}, finished={finished}: {r}");
            assert!(crate::scenario::handle_at("measure:label").is_some());
        }
    }
}

#[test]
fn measurement_dimension_remains_during_view_changes_and_disappears_on_close() {
    let mut h = scene("sketch.arc.center_point", json!({"center": [12.3, 9.7], "start": [16.3, 9.7], "end": [12.3, 13.7]}), true);
    click_world(&mut h, [12.3, 9.7, 0.0]);
    click_world(&mut h, [0.0, 9.7, 0.0]);
    let r = result(&h).clone();
    h.step(&json!({"call": "ui.view", "params": {"view": "iso"}})).unwrap();
    h.settle();
    assert_eq!(*result(&h), r);
    assert!(crate::scenario::handle_at("measure:label").is_some());
    h.step(&json!({"key": "Escape"})).unwrap();
    h.settle();
    assert!(h.app.dialog.is_none());
    assert!(crate::scenario::handle_at("measure:label").is_none());
}

#[test]
fn measurement_preselection_accepts_finished_sketch_geometry() {
    let mut h = scene("sketch.circle.center", json!({"center": [12.3, 9.7], "radius": 4}), true);
    click_world(&mut h, [12.3, 9.7, 0.0]);
    click_world(&mut h, [0.0, 9.7, 0.0]);
    let items = h.app.dialog.as_ref().unwrap().items();
    let measured = result(&h).clone();
    crate::dialogs::cancel(&mut h.app);
    h.app.session.selection = items;
    h.app.start("inspect.measure");
    h.settle();
    assert_eq!(*result(&h), measured);
    assert!(crate::scenario::handle_at("measure:label").is_some());
}
