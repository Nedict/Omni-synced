//! Commands that recompute the history off the window thread (`deferred`): the marker moved,
//! undo, each computed beside the window and run once in; a marker moved again while one move
//! computes replaces the move still waiting. And an agent's control requests (`control_job`,
//! #1989): its design changes computed beside the window, in order, while the window answers.

use std::time::Duration;

use serde_json::json;

use crate::scenario::Harness;

/// A 60 × 20 × 10 block, a 10 × 10 × 5 boss on it, and a 4 mm hole through the boss, the
/// marker at the end. Every command computes off the window thread (no wait at all).
fn block_boss_hole() -> Harness {
    let mut h = Harness::new();
    h.app.home.open = false;
    for (id, p) in [
        ("solid.box", json!({"length": 60, "width": 20, "height": 10})),
        ("solid.box", json!({"corner": [5, 5, 10], "length": 10, "width": 10, "height": 5, "operation": "join"})),
        ("solid.cylinder", json!({"base": [10, 10, -1], "axis": [0, 0, 1], "radius": 2, "height": 20, "operation": "cut"})),
    ] {
        h.app.run(id, p).unwrap();
    }
    h.app.apply_off_thread = Some(Duration::ZERO);
    h
}

/// Frames until nothing computes or waits (a few seconds at most).
fn until_settled(h: &mut Harness) {
    for _ in 0..500 {
        if h.app.deferred.is_none() && h.app.deferred_queue.is_empty() {
            break;
        }
        h.frames(1);
        std::thread::sleep(Duration::from_millis(10));
    }
    h.frames(1);
}

fn marker(h: &Harness) -> Option<usize> {
    h.app.session.doc.marker
}

#[test]
fn the_marker_moved_computes_off_the_window_thread_then_moves() {
    let mut h = block_boss_hole();
    h.app.run_heavy("timeline.roll_to", json!({"position": 1})).unwrap();
    assert!(h.app.computing_since().is_some(), "computing");
    assert_eq!(marker(&h), None, "the design is as it was while it computes");
    until_settled(&mut h);
    assert_eq!(marker(&h), Some(1));
    h.step(&json!({"expect": {"volume": 12000}})).unwrap();
    // And back to the end, the boss and the hole recomputed (or found in the cache).
    h.app.run_heavy("timeline.roll_to", json!({})).unwrap();
    until_settled(&mut h);
    assert_eq!(marker(&h), None);
    let hole = std::f64::consts::PI * 4.0 * 15.0;
    let v = h.app.run("inspect.measure", json!({})).unwrap()["total"]["volume_mm3"].as_f64().unwrap();
    assert!((v - (12500.0 - hole)).abs() < 1.0, "{v}");
}

#[test]
fn a_marker_moved_again_replaces_the_move_still_waiting() {
    let mut h = block_boss_hole();
    h.app.run_heavy("timeline.roll_to", json!({"position": 1})).unwrap();
    assert!(h.app.computing_since().is_some());
    // A drag: two more slots while the first computes. Only the last one waits.
    h.app.run_heavy("timeline.roll_to", json!({"position": 2})).unwrap();
    h.app.run_heavy("timeline.roll_to", json!({"position": 3})).unwrap();
    assert_eq!(h.app.deferred_queue.len(), 1, "{:?}", h.app.deferred_queue);
    until_settled(&mut h);
    assert_eq!(marker(&h), None, "where the drag ended: the end of the history");
}

#[test]
fn undo_computes_off_the_window_thread() {
    let mut h = block_boss_hole();
    let features = h.app.session.doc.features.len();
    h.app.start("edit.undo");
    assert!(h.app.computing_since().is_some(), "undo computing");
    assert_eq!(h.app.session.doc.features.len(), features, "nothing undone yet");
    until_settled(&mut h);
    assert_eq!(h.app.session.doc.features.len(), features - 1, "the hole undone");
    h.step(&json!({"expect": {"volume": 12500}})).unwrap();
    h.app.start("edit.redo");
    until_settled(&mut h);
    assert_eq!(h.app.session.doc.features.len(), features, "and redone");
}

#[test]
fn a_design_changed_while_a_move_computes_is_computed_again() {
    let mut h = block_boss_hole();
    h.app.run_heavy("timeline.roll_to", json!({"position": 2})).unwrap();
    assert!(h.app.computing_since().is_some());
    // Something else changes the design meanwhile (a command that just runs).
    h.app.run("solid.box", json!({"corner": [40, 5, 10], "length": 5, "width": 5, "height": 5, "operation": "join"})).unwrap();
    let revision = h.app.session.revision;
    until_settled(&mut h);
    assert!(h.app.session.revision > revision, "the move ran after the change");
    assert_eq!(marker(&h), Some(2));
}

/// A control request for the app, and where its reply lands.
fn ask(tx: &std::sync::mpsc::Sender<crate::ControlRequest>, method: &str, params: serde_json::Value) -> std::sync::mpsc::Receiver<serde_json::Value> {
    let (req, reply) = crate::ControlRequest::new(method, params);
    tx.send(req).unwrap();
    reply
}

/// The block with its boss and hole, driven over the control channel.
fn agent_harness() -> (Harness, std::sync::mpsc::Sender<crate::ControlRequest>) {
    let mut h = block_boss_hole();
    let (tx, rx) = std::sync::mpsc::channel();
    h.app.control_rx = Some(rx);
    (h, tx)
}

/// Frames until `reply` lands (a few seconds at most).
fn reply_to(h: &mut Harness, reply: &std::sync::mpsc::Receiver<serde_json::Value>) -> serde_json::Value {
    for _ in 0..500 {
        if let Ok(v) = reply.try_recv() {
            return v;
        }
        h.frames(1);
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no reply");
}

fn second_hole() -> serde_json::Value {
    json!({"command": "solid.cylinder", "params": {"base": [50, 10, -1], "axis": [0, 0, 1], "radius": 3, "height": 20, "operation": "cut"}})
}

/// #1989: an agent's design change computes off the window thread. The window keeps answering
/// (`ui.*`, inspection) while it computes and shows which command it is; the agent's next
/// design change waits its turn; each lands as its own undo step with its usual reply.
#[test]
fn an_agent_design_change_computes_off_the_window_thread() {
    let (mut h, tx) = agent_harness();
    let features = h.app.session.doc.features.len();
    let undo = h.app.session.undo.len();
    let hole = ask(&tx, "engine.execute", second_hole());
    let view = ask(&tx, "ui.inspect", json!({}));
    let inspect = ask(&tx, "document.inspect", json!({}));
    let boss = ask(
        &tx,
        "engine.execute",
        json!({"command": "solid.box", "params": {"corner": [40, 5, 10], "length": 5, "width": 5, "height": 5, "operation": "join"}}),
    );
    h.frames(1);
    assert!(h.app.agent_computing().is_some_and(|(_, label)| !label.is_empty()), "the window says the agent's command computes");
    assert!(view.try_recv().is_ok_and(|v| v["ok"] == true), "ui.* answered while it computes");
    let seen = inspect.try_recv().expect("inspection answered while it computes");
    assert_eq!(seen["result"]["timeline"].as_array().map(Vec::len), Some(features), "the design as it was: {seen}");
    assert!(boss.try_recv().is_err(), "the next design change waits its turn");
    assert_eq!(h.app.session.doc.features.len(), features, "nothing applied yet");
    let r = reply_to(&mut h, &hole);
    assert_eq!(r["ok"], true, "{r}");
    let r = reply_to(&mut h, &boss);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(h.app.session.doc.features.len(), features + 2);
    assert_eq!(h.app.session.undo.len(), undo + 2, "one undo step each");
    assert!(h.app.agent_computing().is_none());
    // In the order they were asked for: undo takes the boss, the hole stays.
    h.app.run("edit.undo", json!({})).unwrap();
    let holes = std::f64::consts::PI * (4.0 * 15.0 + 9.0 * 10.0);
    let v = h.app.run("inspect.measure", json!({})).unwrap()["total"]["volume_mm3"].as_f64().unwrap();
    assert!((v - (12500.0 - holes)).abs() < 1.0, "{v}");
}

/// A person's edit while the agent's command computes goes first; the agent's command is
/// computed again for the design as it is and lands after it, never in between.
#[test]
fn a_persons_edit_meanwhile_goes_first_and_the_agents_lands_after() {
    let (mut h, tx) = agent_harness();
    let features = h.app.session.doc.features.len();
    let hole = ask(&tx, "engine.execute", second_hole());
    h.frames(1);
    assert!(h.app.agent_computing().is_some());
    h.app.run("solid.box", json!({"corner": [40, 5, 10], "length": 5, "width": 5, "height": 5, "operation": "join"})).unwrap();
    let r = reply_to(&mut h, &hole);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(h.app.session.doc.features.len(), features + 2, "both kept");
    // Undo takes the agent's hole first: it came last.
    h.app.run("edit.undo", json!({})).unwrap();
    assert_eq!(h.app.session.doc.features.len(), features + 1);
    let boss = 12500.0 + 125.0 - std::f64::consts::PI * 4.0 * 15.0;
    let v = h.app.run("inspect.measure", json!({})).unwrap()["total"]["volume_mm3"].as_f64().unwrap();
    assert!((v - boss).abs() < 1.0, "the person's boss stays: {v}");
}

/// A design replaced while the agent's command computes (another tab, a new design) is not
/// where it lands: it is refused, saying so, and nothing is applied.
#[test]
fn an_agent_command_is_refused_when_its_design_was_replaced() {
    let (mut h, tx) = agent_harness();
    let hole = ask(&tx, "engine.execute", second_hole());
    h.frames(1);
    assert!(h.app.agent_computing().is_some());
    h.app.run("file.new", json!({})).unwrap();
    let r = reply_to(&mut h, &hole);
    assert_eq!(r["ok"], false, "{r}");
    assert!(r["error"].as_str().is_some_and(|e| e.contains("replaced") && e.contains("nothing was applied")), "{r}");
    assert!(h.app.session.doc.features.is_empty(), "nothing went into the new design");
}

/// Stop answers the agent at once, applies nothing, and lets the requests behind it go on.
#[test]
fn stopping_an_agent_command_applies_nothing_and_goes_on() {
    let (mut h, tx) = agent_harness();
    let features = h.app.session.doc.features.len();
    let hole = ask(&tx, "engine.execute", second_hole());
    let boss = ask(
        &tx,
        "engine.execute",
        json!({"command": "solid.box", "params": {"corner": [40, 5, 10], "length": 5, "width": 5, "height": 5, "operation": "join"}}),
    );
    h.frames(1);
    assert!(h.app.agent_computing().is_some());
    h.app.stop_agent_command();
    let r = hole.try_recv().expect("answered at once");
    assert!(r["ok"] == false && r["stopped"] == true, "{r}");
    let r = reply_to(&mut h, &boss);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(h.app.session.doc.features.len(), features + 1, "only the boss");
}

/// A batch computes off the window thread too, with the reply a batch run here gives.
#[test]
fn an_agent_batch_computes_off_the_window_thread() {
    let (mut h, tx) = agent_harness();
    let features = h.app.session.doc.features.len();
    let steps = json!({"commands": [second_hole(), {"command": "solid.box", "params": {"corner": [40, 5, 10], "length": 5, "width": 5, "height": 5, "operation": "join"}}]});
    let batch = ask(&tx, "engine.batch", steps);
    h.frames(1);
    assert!(h.app.agent_computing().is_some(), "computing");
    assert_eq!(h.app.session.doc.features.len(), features);
    let r = reply_to(&mut h, &batch);
    assert_eq!((&r["ok"], &r["result"]["completed"], &r["result"]["undone"]), (&json!(true), &json!(2), &json!(0)), "{r}");
    assert_eq!(h.app.session.doc.features.len(), features + 2);
}

/// An export changes nothing in the design: it runs on a copy and answers from there, the file
/// written; a design change asked for meanwhile does not wait for it.
#[test]
fn an_agent_export_runs_on_a_copy() {
    let (mut h, tx) = agent_harness();
    let dir = std::env::temp_dir().join(format!("sc-agent-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("part.stl").to_string_lossy().to_string();
    let export = ask(&tx, "engine.execute", json!({"command": "file.export", "params": {"path": path}}));
    let r = reply_to(&mut h, &export);
    assert_eq!(r["ok"], true, "{r}");
    assert!(std::fs::metadata(&path).is_ok_and(|m| m.len() > 0), "written");
    assert!(h.app.session.log.iter().any(|l| l.contains("Export")), "echoed as when it runs here: {:?}", h.app.session.log);
    let _ = std::fs::remove_dir_all(&dir);
}

/// With the preference off (Preferences > AI) an agent's design change runs on the window
/// thread, as before: answered in the frame it arrives.
#[test]
fn with_the_preference_off_an_agent_command_runs_at_once() {
    let (mut h, tx) = agent_harness();
    h.app.preferences.agent_off_thread = false;
    let features = h.app.session.doc.features.len();
    let hole = ask(&tx, "engine.execute", second_hole());
    h.frames(1);
    assert!(h.app.agent_computing().is_none());
    assert!(hole.try_recv().is_ok_and(|r| r["ok"] == true), "answered in the same frame");
    assert_eq!(h.app.session.doc.features.len(), features + 1);
}
