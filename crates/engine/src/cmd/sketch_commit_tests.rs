//! `sketch.dimension.commit`: typed values become dimensions, and hostile input changes nothing.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

fn line() -> (Session, String) {
    let mut s = Session::default();
    run(&mut s, "sketch.create", json!({"plane": "XY"}));
    let made = run(&mut s, "sketch.line", json!({"points": [[0.0, 0.0], [10.0, 0.0]], "infer": false}));
    let id = made["curves"][0].as_str().expect("curve id").to_string();
    (s, id)
}

/// Feature: ux-ui-shell-improvements, Property 6: Confirming a typed dimension applies that value
#[test]
fn property_6_confirming_a_typed_dimension_applies_that_value() {
    let mut cases = 0usize;
    for n in 1..=100 {
        let (mut s, id) = line();
        let expression = if n % 2 == 0 { format!("{n}") } else { format!("{n} mm") };
        let v = run(&mut s, "sketch.dimension.commit", json!({"entities": [id], "type": "length", "value": expression}));
        let measured = v["measured"].as_f64().expect("measured");
        assert!((measured - f64::from(n)).abs() < 1e-4, "value {n} measured {measured}");
        let param = v["param"].as_str().expect("param").to_string();
        let edited = run(&mut s, "sketch.dimension.commit", json!({"param": param, "value": format!("{} mm", n + 1)}));
        assert!(edited["expression"].as_str().is_some_and(|e| e.contains(&(n + 1).to_string())));
        cases += 1;
    }
    assert!(cases >= 100, "property ran {cases} times");
}

/// Feature: ux-ui-shell-improvements, Property 8: Hostile dimension input is rejected and the design is unchanged
#[test]
fn property_8_hostile_dimension_input_is_rejected_and_the_design_is_unchanged() {
    let (mut s, id) = line();
    let mut cases = 0usize;
    for n in 0..120 {
        let value = match n % 12 {
            0 => Value::Null,
            1 => json!(""),
            2 => json!("   "),
            3 => json!("NaN"),
            4 => json!("inf"),
            5 => json!("-inf"),
            6 => json!("Infinity"),
            7 => json!(1e12 + f64::from(n)),
            8 => json!("a".repeat(5000)),
            9 => json!(format!("not-a-number-{n}")),
            10 => json!({"bad": true}),
            _ => json!(format!("{n}e20")),
        };
        let before = s.doc.to_json();
        let undo = s.undo.len();
        let err = s.execute("sketch.dimension.commit", &json!({"entities": [id], "value": value}));
        assert!(err.is_err(), "case {n} should fail");
        assert_eq!(s.doc.to_json(), before, "case {n} changed the design");
        assert_eq!(s.undo.len(), undo, "case {n} changed undo");
        cases += 1;
    }
    assert!(cases >= 100, "property ran {cases} times");
}
