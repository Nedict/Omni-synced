//! A blind pocket must stay a void when a later through cut passes the same body.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap_or_else(|e| panic!("{command} {params}: {e}"))
}

fn volume(s: &mut Session, name: &str) -> f64 {
    run(s, "inspect.measure", json!({ "bodies": [name] }))["bodies"][0]["volume_mm3"].as_f64().unwrap()
}

fn body(s: &Session, name: &str) -> solidcraft_kernel::Body {
    s.model.state().bodies.iter().find(|b| b.name == name).unwrap_or_else(|| panic!("missing {name}")).body.clone()
}

/// Box, a blind pocket, then a through slot beside it. The pocket void stays, a second body
/// is untouched, and undo puts the pocket back.
#[test]
fn blind_pocket_survives_a_through_slot() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 80, "width": 50, "height": 20, "body_name": "Plate"}));
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": [300, 0, 0], "operation": "new", "body_name": "Witness"}));
    let witness = volume(&mut s, "Witness");
    assert!((witness - 1000.0).abs() < 1e-6, "{witness}");

    run(&mut s, "sketch.create", json!({"plane": {"face": [40, 25, 20]}, "name": "Pocket"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [10, 10], "p1": [30, 26]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"sketch": "Pocket", "distance": 8, "direction": "negative", "operation": "cut", "targets": ["Plate"]}));
    let pocket = 20.0 * 16.0 * 8.0;
    let after_pocket = 80.0 * 50.0 * 20.0 - pocket;
    let plate = volume(&mut s, "Plate");
    assert!((plate - after_pocket).abs() < 1e-4, "pocket {plate} vs {after_pocket}");
    assert!(!body(&s, "Plate").contains(solidcraft_geom::Vec3::new(20.0, 18.0, 16.0)), "pocket void was filled");
    assert!(body(&s, "Plate").contains(solidcraft_geom::Vec3::new(20.0, 18.0, 6.0)), "material under the pocket floor vanished");
    assert_eq!(body(&s, "Plate").shell_count(), 1, "the pocket must not become a second shell");

    run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Slot"}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [50, 5], "p1": [62, 45]}));
    run(&mut s, "sketch.finish", json!({}));
    let faces_before = body(&s, "Plate").face_count();
    run(&mut s, "solid.extrude", json!({"sketch": "Slot", "through_all": true, "operation": "cut", "targets": ["Plate"]}));
    let slot = 12.0 * 40.0 * 20.0;
    let expected = after_pocket - slot;
    let plate = volume(&mut s, "Plate");
    assert!((plate - expected).abs() < 1e-3, "through slot {plate} vs {expected}");
    assert!((volume(&mut s, "Witness") - witness).abs() < 1e-9, "the other body changed");
    let plate_body = body(&s, "Plate");
    assert!(!plate_body.contains(solidcraft_geom::Vec3::new(20.0, 18.0, 16.0)), "through cut filled the pocket");
    assert!(plate_body.contains(solidcraft_geom::Vec3::new(20.0, 18.0, 6.0)), "through cut removed the pocket floor");
    assert!(!plate_body.contains(solidcraft_geom::Vec3::new(56.0, 25.0, 10.0)), "slot did not pass through");
    assert_eq!(plate_body.shell_count(), 1, "through cut invented an inner shell");
    assert!(plate_body.face_count() > faces_before, "the slot added no faces");
    assert!(plate_body.face_count() < faces_before + 8, "through cut invented faces beyond the slot walls");

    run(&mut s, "edit.undo", json!({}));
    let restored = volume(&mut s, "Plate");
    assert!((restored - after_pocket).abs() < 1e-4, "undo {restored} vs {after_pocket}");
    assert!(!body(&s, "Plate").contains(solidcraft_geom::Vec3::new(20.0, 18.0, 16.0)));
    assert!((volume(&mut s, "Witness") - witness).abs() < 1e-9);
}
