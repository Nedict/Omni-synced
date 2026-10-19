//! The Extrude window's Targets: under Operation (none for a New Body), a pill per body. A new
//! Join, Cut or Intersect lists the bodies it changes, the visible ones, following what is
//! shown until the list is edited by hand; what is listed is what the feature stores.
use crate::dialogs::{Kind, TARGETS, apply_commands};
use crate::scenario::Harness;
use serde_json::{Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::FeatureKind;

/// A 40 x 40 x 5 plate, a copy of it where it is, and a rim sketched over the plate's top: on
/// the plate's face, or on a plane at its height.
fn plate_and_copy(on_face: bool) -> Harness {
    let mut h = Harness::new();
    h.app.run("solid.box", json!({"length":40,"width":40,"height":5,"body_name":"Plate"})).unwrap();
    h.app.run("solid.move", json!({"bodies":["Plate"],"copy":true})).unwrap();
    let plane = if on_face {
        let st = h.app.session.world_state();
        let faces = st.body("Plate").unwrap().body.faces(0.01).unwrap();
        let top = faces.into_iter().find(|f| f.plane_normal.is_some_and(|n| n.z > 0.99)).unwrap();
        json!({"face":[20,20,5],"body":"Plate","index":top.index})
    } else {
        json!({"origin":[0,0,5],"x_dir":[1,0,0],"y_dir":[0,1,0]})
    };
    h.app.run("sketch.create", json!({"plane":plane,"name":"Rim"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0":[2,2],"p1":[38,38]})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0":[4,4],"p1":[36,36]})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    h
}

fn hide(h: &mut Harness, body: &str) {
    h.app.run("browser.visibility", json!({"items":[body],"visible":false})).unwrap();
}

/// The extrude window on the rim's ring, joining.
fn start_join(h: &mut Harness) {
    let ring = rim_ring(h);
    h.app.session.selection = vec![ring];
    h.app.start("solid.extrude");
    let d = h.app.dialog.as_mut().unwrap();
    if let Kind::Extrude { distance, operation, auto_op, .. } = &mut d.kind {
        *distance = "3 mm".into();
        *operation = 1;
        *auto_op = false;
    }
    h.frames(2);
}

fn rim_ring(h: &Harness) -> Sel {
    let st = h.app.session.world_state();
    let ss = st.sketches.iter().find(|ss| ss.name == "Rim").unwrap();
    let index = ss.profiles.iter().position(|p| !p.region.holes.is_empty()).unwrap();
    Sel::Profile { sketch: ss.feature, index }
}

fn params(h: &Harness) -> Value {
    apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap().remove(0).1
}

fn volume(h: &Harness, body: &str) -> f64 {
    solidcraft_engine::doc::kernel::measure(&h.app.session.model.state().body(body).unwrap().body).unwrap().volume
}

const RIM: f64 = (36.0 * 36.0 - 32.0 * 32.0) * 3.0;

/// The reported case: a copy hidden, the join goes into the plate alone and the copy stays.
#[test]
fn a_join_with_a_body_hidden_targets_the_visible_one() {
    let mut h = plate_and_copy(false);
    hide(&mut h, "Plate (2)");
    start_join(&mut h);
    assert_eq!(params(&h)["targets"], json!(["Plate"]));
    h.step(&json!({"key":"Enter"})).unwrap();
    assert!(h.app.dialog.is_none(), "{:?}", h.app.dialog);
    assert!((volume(&h, "Plate") - (8000.0 + RIM)).abs() < 1e-6);
    assert!((volume(&h, "Plate (2)") - 8000.0).abs() < 1e-6);
    // Showing the copy again changes nothing: the targets are the feature's.
    h.app.run("browser.visibility", json!({"items":["Plate (2)"],"visible":true})).unwrap();
    assert_eq!(h.app.session.model.state().bodies.len(), 2);
    let FeatureKind::Extrude { targets, .. } = &h.app.session.doc.features.last().unwrap().kind else { panic!("an extrude") };
    assert_eq!(targets.iter().map(|t| t.to_string()).collect::<Vec<_>>(), ["Plate"]);
}

/// Hiding the other one (the copy shown) targets the copy, even from a sketch on the plate's face.
#[test]
fn a_join_from_a_hidden_bodys_face_targets_the_visible_one() {
    let mut h = plate_and_copy(true);
    hide(&mut h, "Plate");
    start_join(&mut h);
    assert_eq!(params(&h)["targets"], json!(["Plate (2)"]));
    h.step(&json!({"key":"Enter"})).unwrap();
    assert!(volume(&h, "Plate (2)") > 8000.0 + 100.0, "the copy is joined");
    assert!((volume(&h, "Plate") - 8000.0).abs() < 1e-6);
}

/// Every body shown: all of them are listed and stored. A sketch on a shown body's face joins
/// that body alone, as the command does.
#[test]
fn the_list_holds_the_visible_bodies_or_the_face_body() {
    let mut h = plate_and_copy(false);
    start_join(&mut h);
    assert_eq!(params(&h)["targets"], json!(["Plate", "Plate (2)"]));
    let mut h = plate_and_copy(true);
    hide(&mut h, "Plate (2)");
    start_join(&mut h);
    assert_eq!(params(&h)["targets"], json!(["Plate"]));
    h.step(&json!({"key":"Enter"})).unwrap();
    assert!((volume(&h, "Plate (2)") - 8000.0).abs() < 1e-6, "the sketch's face body is joined alone");
}

/// The design active, its plate kept in a component: the plate is listed. A component of its
/// own, empty: nothing is, and Auto makes a new body there rather than joining the other
/// component's plate.
#[test]
fn bodies_of_other_components_are_listed_only_from_the_design() {
    let mut h = Harness::new();
    h.app.run("component.create", json!({"name":"Base"})).unwrap();
    h.app.run("solid.box", json!({"length":40,"width":40,"height":5,"body_name":"Plate"})).unwrap();
    h.app.run("component.activate", json!({"component":0})).unwrap();
    h.app.run("sketch.create", json!({"plane":{"origin":[0,0,5],"x_dir":[1,0,0],"y_dir":[0,1,0]},"name":"Rim"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0":[2,2],"p1":[38,38]})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0":[4,4],"p1":[36,36]})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    start_join(&mut h);
    assert_eq!(params(&h)["targets"], json!(["Plate"]));
    h.app.dialog = None;
    h.app.run("component.create", json!({"name":"Lid"})).unwrap();
    h.app.run("sketch.create", json!({"plane":{"origin":[0,0,30],"x_dir":[1,0,0],"y_dir":[0,1,0]},"name":"Top"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,40]})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    let st = h.app.session.world_state();
    let top = st.sketches.iter().find(|ss| ss.name == "Top").unwrap().feature;
    h.app.session.selection = vec![Sel::Profile { sketch: top, index: 0 }];
    h.app.start("solid.extrude");
    h.frames(2);
    assert!(h.app.dialog.as_ref().unwrap().inputs.iter().find(|i| i.label == TARGETS).is_some_and(|i| i.items.is_empty()));
    let p = params(&h);
    assert_eq!((p["operation"].as_str(), p.get("targets")), (Some("new"), None), "{p}");
    h.step(&json!({"key":"Enter"})).unwrap();
    assert_eq!(h.app.session.model.state().bodies.len(), 2);
}

/// A box named `body` in a new component `name` (made under the active one, and left active).
fn part(h: &mut Harness, name: &str, body: &str, x: f64) {
    h.app.run("component.create", json!({"name":name})).unwrap();
    h.app.run("solid.box", json!({"corner":[x,0,0],"length":20,"width":20,"height":5,"body_name":body})).unwrap();
}

/// A rectangle sketched on a plane at height `z`, from `p0` to `p1`, picked as the profile.
fn sketch_profile(h: &mut Harness, z: f64, p0: [f64; 2], p1: [f64; 2]) {
    h.app.run("sketch.create", json!({"plane":{"origin":[0,0,z],"x_dir":[1,0,0],"y_dir":[0,1,0]},"name":"Pick"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0":p0,"p1":p1})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    let st = h.app.session.world_state();
    let sketch = st.sketches.iter().find(|ss| ss.name == "Pick").unwrap().feature;
    h.app.session.selection = vec![Sel::Profile { sketch, index: 0 }];
}

/// The window's Auto on what is selected: the operation it lands on and the targets it sends.
fn auto(h: &mut Harness) -> (String, Value) {
    h.app.start("solid.extrude");
    h.frames(3);
    let p = params(h);
    (p["operation"].as_str().unwrap_or_default().to_string(), p.get("targets").cloned().unwrap_or(Value::Null))
}

/// What the window starts with is what the command takes on its own, less hidden bodies: from
/// the design with parts in two components a new body, not a lump in one of them; a face of
/// another component's body picked in an empty component a new body; with hidden bodies only a
/// new body, not a refusal; a Cut from an empty component every shown body.
#[test]
fn the_default_is_what_the_command_takes() {
    let mut h = Harness::new();
    part(&mut h, "A", "ABox", 0.0);
    h.app.run("component.activate", json!({"component":0})).unwrap();
    part(&mut h, "B", "BBox", 40.0);
    h.app.run("component.activate", json!({"component":0})).unwrap();
    sketch_profile(&mut h, 0.0, [100.0, 100.0], [110.0, 110.0]);
    assert_eq!(auto(&mut h), ("new".to_string(), Value::Null), "two components: a new body at the design");
    h.app.dialog = None;

    let mut h = Harness::new();
    part(&mut h, "Base", "Plate", 0.0);
    h.app.run("component.activate", json!({"component":0})).unwrap();
    h.app.run("component.create", json!({"name":"Lid"})).unwrap();
    let st = h.app.session.world_state();
    let top = st.body("Plate").unwrap().body.faces(0.01).unwrap().into_iter().find(|f| f.plane_normal.is_some_and(|n| n.z > 0.99)).unwrap();
    h.app.session.selection = vec![Sel::Face { body: "Plate".into(), index: top.index, point: top.centroid }];
    assert_eq!(auto(&mut h).0, "new", "a face of another component's body, from an empty component");
    h.app.dialog = None;

    let mut h = Harness::new();
    h.app.run("solid.box", json!({"length":20,"width":20,"height":5,"body_name":"Hidden"})).unwrap();
    hide(&mut h, "Hidden");
    sketch_profile(&mut h, 5.0, [2.0, 2.0], [8.0, 8.0]);
    assert_eq!(auto(&mut h), ("new".to_string(), Value::Null), "hidden bodies only");
    h.step(&json!({"key":"Enter"})).unwrap();
    assert_eq!(h.app.session.model.state().bodies.len(), 2, "OK makes the new body");

    let mut h = Harness::new();
    h.app.run("component.create", json!({"name":"Sub"})).unwrap();
    part(&mut h, "A", "ABox", 0.0);
    h.app.run("component.activate", json!({"component":"Sub"})).unwrap();
    part(&mut h, "B", "BBox", 30.0);
    h.app.run("component.activate", json!({"component":"Sub"})).unwrap();
    sketch_profile(&mut h, 5.0, [10.0, 5.0], [40.0, 15.0]);
    h.app.start("solid.extrude");
    if let Kind::Extrude { operation, auto_op, distance, .. } = &mut h.app.dialog.as_mut().unwrap().kind {
        (*operation, *auto_op, *distance) = (2, false, "-2 mm".into());
    }
    h.frames(3);
    assert_eq!(params(&h)["targets"], json!(["ABox", "BBox"]), "a Cut from an empty component takes every body");
}

/// Showing or hiding a body while the window is open changes the list, until a pill is removed
/// by hand: then it stays as edited.
#[test]
fn the_list_follows_visibility_until_edited() {
    let mut h = plate_and_copy(false);
    start_join(&mut h);
    hide(&mut h, "Plate (2)");
    h.frames(2);
    assert_eq!(params(&h)["targets"], json!(["Plate"]));
    h.app.run("browser.visibility", json!({"items":["Plate (2)"],"visible":true})).unwrap();
    h.frames(2);
    assert_eq!(params(&h)["targets"], json!(["Plate", "Plate (2)"]));
    h.step(&json!({"click":{"handle":"dialog:target:Plate (2)"}})).unwrap();
    h.frames(2);
    assert_eq!(params(&h)["targets"], json!(["Plate"]), "the pill's × takes it out");
    hide(&mut h, "Plate");
    h.frames(2);
    assert_eq!(params(&h)["targets"], json!(["Plate"]), "an edited list stays");
}

/// The row sits under Operation and is not there for a New Body; an empty list is refused.
#[test]
fn the_row_is_under_operation_and_not_for_a_new_body() {
    let mut h = plate_and_copy(false);
    start_join(&mut h);
    let (op, row) = (crate::scenario::handle_at("dialog:Operation").unwrap(), crate::scenario::handle_at("dialog:Targets").unwrap());
    assert!(row.y > op.y, "Targets {row:?} under Operation {op:?}");
    assert!(crate::scenario::handle_at("dialog:target:Plate").is_some(), "a pill per body");
    if let Kind::Extrude { operation, .. } = &mut h.app.dialog.as_mut().unwrap().kind {
        *operation = 0;
    }
    crate::scenario::clear_handles();
    h.frames(2);
    assert!(crate::scenario::handle_at("dialog:Targets").is_none(), "no Targets for a new body");
    assert!(params(&h).get("targets").is_none());
    if let Kind::Extrude { operation, .. } = &mut h.app.dialog.as_mut().unwrap().kind {
        *operation = 1;
    }
    h.frames(2);
    for name in ["Plate", "Plate (2)"] {
        h.step(&json!({"click":{"handle":format!("dialog:target:{name}")}})).unwrap();
        h.frames(2);
    }
    let refused = apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap_err();
    assert!(refused.contains("New Body"), "{refused}");
}

/// Clicking a listed body in the viewport takes it out; what is left is what the join changes,
/// and Edit shows and keeps it.
#[test]
fn picked_targets_are_sent_and_kept_when_editing() {
    let mut h = Harness::new();
    h.app.run("solid.box", json!({"length":40,"width":40,"height":5,"body_name":"Plate"})).unwrap();
    h.app.run("solid.box", json!({"length":40,"width":40,"height":5,"corner":[0,0,10],"operation":"new","body_name":"Ref"})).unwrap();
    h.app.run("sketch.create", json!({"plane":{"origin":[0,0,5],"x_dir":[1,0,0],"y_dir":[0,1,0]},"name":"Rim"})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0":[2,2],"p1":[38,38]})).unwrap();
    h.app.run("sketch.rectangle.two_point", json!({"p0":[4,4],"p1":[36,36]})).unwrap();
    h.app.run("sketch.finish", json!({})).unwrap();
    start_join(&mut h);
    let d = h.app.dialog.as_ref().unwrap();
    assert_eq!(d.active, 0, "the profiles stay the active input");
    let index = d.inputs.iter().position(|i| i.label == TARGETS).unwrap();
    assert_eq!(d.inputs[index].items, vec![Sel::Body { name: "Plate".into() }, Sel::Body { name: "Ref".into() }]);
    h.call("ui.dialogInput", json!({"index":index}));
    h.call("ui.view", json!({"view":"front"}));
    h.frames(2);
    h.step(&json!({"click":{"world":[20,0,12.5]}})).unwrap();
    assert_eq!(h.app.dialog.as_ref().unwrap().inputs[index].items, vec![Sel::Body { name: "Plate".into() }]);
    assert_eq!(params(&h)["targets"], json!(["Plate"]));
    h.step(&json!({"key":"Enter"})).unwrap();
    assert!(h.app.dialog.is_none(), "{:?}", h.app.dialog);
    let feature = h.app.session.doc.features.last().unwrap().id;
    let before = serde_json::to_value(&h.app.session.doc.feature(feature).unwrap().kind).unwrap();
    h.app.edit_feature(feature);
    h.frames(2);
    let d = h.app.dialog.as_ref().unwrap();
    let index = d.inputs.iter().position(|i| i.label == TARGETS).unwrap();
    assert_eq!(d.inputs[index].items, vec![Sel::Body { name: "Plate".into() }]);
    let edited = apply_commands(&h.app, d).unwrap();
    assert_eq!(edited[0].1["params"]["targets"], json!(["Plate"]));
    h.step(&json!({"key":"Enter"})).unwrap();
    assert!(h.app.dialog.is_none(), "{:?}", h.app.dialog);
    assert_eq!(serde_json::to_value(&h.app.session.doc.feature(feature).unwrap().kind).unwrap(), before);
}

/// Editing a feature made with no targets keeps it that way, whatever is hidden now.
#[test]
fn editing_keeps_a_feature_without_targets_as_it_is() {
    let mut h = plate_and_copy(false);
    h.app.run("solid.extrude", json!({"sketch":"Rim","profiles":[0],"distance":3,"operation":"join","name":"Lip"})).unwrap();
    let feature = h.app.session.doc.find_feature("Lip").unwrap().id;
    hide(&mut h, "Plate");
    h.app.edit_feature(feature);
    h.frames(2);
    let edited = apply_commands(&h.app, h.app.dialog.as_ref().unwrap()).unwrap();
    assert!(edited[0].1["params"].get("targets").is_none(), "{}", edited[0].1);
}

/// A face picked before Extrude is still extruded (Press Pull style), not taken as a target.
#[test]
fn a_preselected_face_stays_the_profile_and_a_preselected_body_is_the_target() {
    let mut h = Harness::new();
    h.app.run("solid.box", json!({"length":20,"width":20,"height":10,"body_name":"Base"})).unwrap();
    let st = h.app.session.world_state();
    let base = st.body("Base").unwrap();
    let top = base.body.faces(0.01).unwrap().into_iter().find(|f| f.plane_normal.is_some_and(|n| n.z > 0.99)).unwrap();
    h.app.session.selection = vec![Sel::Face { body: "Base".into(), index: top.index, point: top.centroid }];
    h.app.start("solid.extrude");
    let d = h.app.dialog.as_ref().unwrap();
    assert!(matches!(d.inputs[0].items.first(), Some(Sel::Face { .. })), "{:?}", d.inputs);
    assert_eq!(d.inputs.iter().find(|i| i.label == TARGETS).unwrap().items, vec![Sel::Body { name: "Base".into() }], "the face's body");
    h.app.dialog = None;
    h.app.session.selection = vec![Sel::Body { name: "Base".into() }];
    h.app.start("solid.extrude");
    let d = h.app.dialog.as_ref().unwrap();
    assert_eq!(d.inputs.iter().find(|i| i.label == TARGETS).unwrap().items, vec![Sel::Body { name: "Base".into() }]);
}
