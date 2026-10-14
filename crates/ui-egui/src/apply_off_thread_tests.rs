//! OK off the window thread: a feature that takes longer than the app's wait is computed beside
//! the window, the dialog saying so, and applied once it is in; Cancel drops it.

use std::time::Duration;

use serde_json::json;

use crate::scenario::Harness;

/// A 60 × 20 × 10 block and, just finished on its top, a 10 × 10 square: Extrude takes it.
fn square_on_a_block() -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    for (id, p) in [
        ("sketch.create", json!({"plane": "XY"})),
        ("sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [60, 20]})),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"distance": 10})),
        ("sketch.create", json!({"plane": {"origin": [0, 0, 10], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}})),
        ("sketch.rectangle.two_point", json!({"p0": [5, 5], "p1": [15, 15]})),
    ] {
        h.app.run(id, p).unwrap();
    }
    h.app.finish_sketch();
    h.app.start("solid.extrude");
    h.frames(2);
    h
}

/// Frames until OK's evaluation is in and applied (a few seconds at most).
fn until_applied(h: &mut Harness) {
    for _ in 0..500 {
        if h.app.applying.is_none() {
            break;
        }
        h.frames(1);
        std::thread::sleep(Duration::from_millis(10));
    }
    h.frames(1);
}

#[test]
fn ok_computes_off_the_window_thread_saying_so_then_applies() {
    let mut h = square_on_a_block();
    h.app.apply_off_thread = Some(Duration::ZERO);
    h.step(&json!({"key": "Enter"})).unwrap();
    assert!(h.app.applying.is_some() && h.app.dialog.is_some(), "computing, the dialog open");
    h.frames(1);
    assert!(h.widget("Computing… 0 s").is_some(), "the dialog says it computes");
    // Enter again does not start it twice.
    h.step(&json!({"key": "Enter"})).unwrap();
    until_applied(&mut h);
    assert!(h.app.dialog.is_none(), "{:?}", h.app.dialog.as_ref().and_then(|d| d.error.clone()));
    h.step(&json!({"expect": {"bodies": 1, "volume": 13000}})).unwrap();
    assert_eq!(h.app.session.doc.features.len(), 4, "one extrude added");
}

#[test]
fn cancel_while_computing_applies_nothing() {
    let mut h = square_on_a_block();
    h.app.apply_off_thread = Some(Duration::ZERO);
    let features = h.app.session.doc.features.len();
    h.step(&json!({"key": "Enter"})).unwrap();
    assert!(h.app.applying.is_some());
    h.step(&json!({"key": "Escape"})).unwrap();
    assert!(h.app.applying.is_none() && h.app.dialog.is_none());
    h.frames(5);
    assert_eq!(h.app.session.doc.features.len(), features);
}
