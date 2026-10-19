//! The walls of a box joined round its floor: a ring sketched round the floor's outline, rounded
//! corners and all, extruded as a join from the floor's bottom, or from the floor of a socket
//! under it, past its top. The ring only touches the floor, back to back on its sides, and the
//! floor is no prism (a pocket with sloping sides): the join failed with "Union: no result".

use super::*;

/// A 60 × 50 floor 3.25 thick with r4 corners, a pocket with sloping sides in its top and a
/// socket under it, drawn with the ring round it (10 wide, r14 outside) in its sketch.
fn floor(s: &mut Session) {
    run(s, "sketch.create", json!({"plane": "XY", "name": "Outline"}));
    run(s, "sketch.rectangle.rounded", json!({"p0": [0, 0], "p1": [60, 50], "radius": 4}));
    run(s, "sketch.rectangle.rounded", json!({"p0": [-10, -10], "p1": [70, 60], "radius": 14}));
    run(s, "sketch.finish", json!({}));
    run(s, "solid.extrude", json!({"sketch": "Outline", "profiles": [{"point": [30, 25]}], "distance": 3.25, "body_name": "Floor"}));
    run(s, "sketch.create", json!({"plane": "XY", "offset": 3.25, "name": "Pocket"}));
    run(s, "sketch.rectangle.rounded", json!({"p0": [10, 10], "p1": [50, 40], "radius": 3}));
    run(s, "sketch.finish", json!({}));
    run(
        s,
        "solid.extrude",
        json!({"sketch": "Pocket", "distance": 2.25, "direction": "negative", "taper": "-15 deg", "operation": "cut", "targets": ["Floor"]}),
    );
    run(s, "sketch.create", json!({"plane": "XY", "name": "Socket"}));
    run(s, "sketch.rectangle.two_point", json!({"p0": [12, 12], "p1": [48, 38]}));
    run(s, "sketch.finish", json!({}));
    run(s, "solid.extrude", json!({"sketch": "Socket", "distance": 4.75, "direction": "negative", "operation": "join", "targets": ["Floor"]}));
}

#[test]
fn walls_join_flush_round_a_floor() {
    let ring = (80.0 * 70.0 - (4.0 - PI) * 14.0 * 14.0) - (60.0 * 50.0 - (4.0 - PI) * 4.0 * 4.0);
    // From the floor's bottom, 40 high; from the socket's floor (4.75 under it), 10.
    for (from, height) in [(None, 40.0), (Some(json!([30, 25, -4.75])), 10.0)] {
        let mut s = Session::default();
        floor(&mut s);
        let before = volume(&mut s);
        let mut walls =
            json!({"sketch": "Outline", "profiles": [{"point": [-5, 25]}], "distance": height, "operation": "join", "targets": ["Floor"]});
        if let Some(from) = from {
            walls["from"] = from;
        }
        let reply = run(&mut s, "solid.extrude", walls);
        assert!(reply["warning"].is_null(), "{height}: {reply}");
        let result = s.model.result(reply["feature"].as_u64().unwrap()).unwrap();
        assert!(!result.boolean_diagnostics.events.iter().any(|e| e.strategy.alters_result()), "{height}: {:?}", result.boolean_diagnostics);
        let state = s.model.state();
        assert_eq!(state.bodies.len(), 1);
        let floor = &state.bodies[0].body;
        assert_eq!(floor.shell_count(), 1);
        assert!(floor.validity().is_empty(), "{height}: {:?}", floor.validity());
        let got = volume(&mut s);
        assert!(rel(got - before, ring * height) < 2e-5, "{height}: {} vs {}", got - before, ring * height);
    }
}
