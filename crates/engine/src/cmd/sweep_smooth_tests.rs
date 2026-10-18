//! A circle swept along a straight 3D path, and a path that crosses itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap_or_else(|e| panic!("{command} {params}: {e}"))
}

#[test]
fn circle_swept_along_a_straight_path_has_pipe_volume() {
    let mut s = Session::default();
    let path_sketch = run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Path"}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.options", json!({"sketch_3d": true}));
    let wire = run(&mut s, "sketch.line3d", json!({"points": [[0, 0, 0], [0, 0, 20]]}))["curves"][0].as_str().unwrap().to_string();
    run(&mut s, "sketch.finish", json!({}));
    let profile = run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Profile"}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.circle.center", json!({"center": [0, 0], "radius": 5}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.sweep", json!({"sketch": profile, "path_sketch": path_sketch, "path": [wire], "operation": "new"}));
    let volume = run(&mut s, "inspect.measure", json!({}))["total"]["volume_mm3"].as_f64().unwrap();
    let want = std::f64::consts::PI * 25.0 * 20.0;
    assert!((volume - want).abs() < 5e-4 * want, "{volume} vs {want}");
}

#[test]
fn a_self_intersecting_sweep_path_adds_no_feature() {
    let mut s = Session::default();
    let path_sketch = run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Path"}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.options", json!({"sketch_3d": true}));
    let curves = run(&mut s, "sketch.line3d", json!({"points": [[0, 0, 0], [10, 10, 0], [0, 10, 0], [10, 0, 0]]}))["curves"].clone();
    run(&mut s, "sketch.finish", json!({}));
    let profile = run(&mut s, "sketch.create", json!({"plane": "XY", "name": "Profile"}))["sketch"].as_u64().unwrap();
    run(&mut s, "sketch.circle.center", json!({"center": [0, 0], "radius": 1}));
    run(&mut s, "sketch.finish", json!({}));
    let features = s.doc.features.len();
    let bodies = s.model.state().bodies.len();
    let err = s.execute("solid.sweep", &json!({"sketch": profile, "path_sketch": path_sketch, "path": curves, "operation": "new"})).unwrap_err();
    assert!(err.to_string().contains("not supported yet"), "{err}");
    assert_eq!(s.doc.features.len(), features);
    assert_eq!(s.model.state().bodies.len(), bodies);
}
