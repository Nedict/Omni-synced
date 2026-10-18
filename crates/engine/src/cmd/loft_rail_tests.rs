//! One loft guide rail bows the mid-section. Two rails bow both directions. A rail that
//! misses the sections is refused and adds no feature.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap_or_else(|e| panic!("{command} {params}: {e}"))
}

fn rectangles(s: &mut Session) {
    run(s, "sketch.create", json!({"plane": "XY", "name": "Bottom"}));
    run(s, "sketch.rectangle.two_point", json!({"p0": [-10, -5], "p1": [10, 5]}));
    run(s, "sketch.finish", json!({}));
    run(s, "sketch.create", json!({"plane": {"origin": [0, 0, 40], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}, "name": "Top"}));
    run(s, "sketch.rectangle.two_point", json!({"p0": [-10, -5], "p1": [10, 5]}));
    run(s, "sketch.finish", json!({}));
}

#[test]
fn a_guide_rail_sets_the_mid_section_width() {
    let mut s = Session::default();
    rectangles(&mut s);
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Guide"}));
    let a = run(&mut s, "sketch.line", json!({"points": [[10, 0], [30, 20]]}));
    let b = run(&mut s, "sketch.line", json!({"points": [[30, 20], [10, 40]]}));
    run(&mut s, "sketch.finish", json!({}));
    let curves = [a["curves"][0].clone(), b["curves"][0].clone()];
    run(&mut s, "solid.loft", json!({"sections": [{"sketch": "Bottom"}, {"sketch": "Top"}], "rails": {"sketch": "Guide", "curves": curves}}));
    let body = s.model.state().bodies.last().unwrap().body.clone();
    assert!(body.contains(solidcraft_geom::Vec3::new(25.0, 0.0, 20.0)), "the mid-section should follow the rail out to x=30");
    assert!(!body.contains(solidcraft_geom::Vec3::new(36.0, 0.0, 20.0)), "the mid-section should stop at the rail");
    let m = solidcraft_kernel::measure(&body).unwrap();
    assert!(m.bbox.max.x > 28.0 && m.bbox.max.x < 36.0, "width follows the rail, max x {}", m.bbox.max.x);
}

#[test]
fn a_rail_that_misses_the_sections_adds_no_feature() {
    let mut s = Session::default();
    rectangles(&mut s);
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "Guide"}));
    let a = run(&mut s, "sketch.line", json!({"points": [[40, 8], [40, 16]]}));
    run(&mut s, "sketch.finish", json!({}));
    let features = s.doc.features.len();
    let err = s
        .execute(
            "solid.loft",
            &json!({"sections": [{"sketch": "Bottom"}, {"sketch": "Top"}], "rails": {"sketch": "Guide", "curves": [a["curves"][0].clone()]}}),
        )
        .unwrap_err();
    assert!(err.to_string().contains("misses"), "{err}");
    assert_eq!(s.doc.features.len(), features);
}

#[test]
fn two_guide_rails_set_both_widths() {
    let mut s = Session::default();
    rectangles(&mut s);
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}, "name": "GuideX"}));
    let ax = run(&mut s, "sketch.line", json!({"points": [[10, 0], [30, 20]]}));
    let bx = run(&mut s, "sketch.line", json!({"points": [[30, 20], [10, 40]]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [0, 1, 0], "y_dir": [0, 0, 1]}, "name": "GuideY"}));
    let ay = run(&mut s, "sketch.line", json!({"points": [[5, 0], [20, 20]]}));
    let by = run(&mut s, "sketch.line", json!({"points": [[20, 20], [5, 40]]}));
    run(&mut s, "sketch.finish", json!({}));
    let rail_x = [ax["curves"][0].clone(), bx["curves"][0].clone()];
    let rail_y = [ay["curves"][0].clone(), by["curves"][0].clone()];
    run(
        &mut s,
        "solid.loft",
        json!({"sections": [{"sketch": "Bottom"}, {"sketch": "Top"}], "rails": [{"sketch": "GuideX", "curves": rail_x}, {"sketch": "GuideY", "curves": rail_y}]}),
    );
    let body = s.model.state().bodies.last().unwrap().body.clone();
    assert!(body.contains(solidcraft_geom::Vec3::new(25.0, 0.0, 20.0)), "the mid-section should follow the X rail");
    assert!(body.contains(solidcraft_geom::Vec3::new(0.0, 12.0, 20.0)), "the mid-section should follow the Y rail");
    assert!(!body.contains(solidcraft_geom::Vec3::new(36.0, 0.0, 20.0)));
    assert!(!body.contains(solidcraft_geom::Vec3::new(0.0, 24.0, 20.0)));
}
