use super::*;
use std::sync::mpsc::channel;

fn chat() -> AgentChat {
    let mut c = AgentChat::default();
    c.state = Some(State::Ready);
    c
}

#[test]
fn streamed_text_joins_and_tool_updates_change_their_call() {
    let mut c = chat();
    for e in [
        Event::Text { thought: false, text: "Mak".into() },
        Event::Text { thought: false, text: "ing a box".into() },
        Event::Tool { id: "t".into(), title: Some("batch".into()), kind: Some("other".into()), status: Some(ToolStatus::Pending), output: None },
        Event::Tool { id: "t".into(), title: None, kind: None, status: Some(ToolStatus::Completed), output: Some("ok".into()) },
    ] {
        c.on_event(e);
    }
    assert_eq!(c.items.len(), 2);
    assert!(matches!(&c.items[0], Item::Agent { text, .. } if text == "Making a box"));
    assert!(matches!(&c.items[1], Item::Tool { title, status: Some(ToolStatus::Completed), output: Some(o), .. } if title == "batch" && o == "ok"));
}

#[test]
fn a_sign_in_request_keeps_the_question_for_after_the_login() {
    let mut c = chat();
    c.last = Some(("make a cube".into(), false));
    c.state = Some(State::Busy);
    assert!(c.on_event(Event::TurnEnded { reason: "auth".into() }).is_some());
    c.on_event(Event::AuthRequired { methods: vec![] });
    assert_eq!(c.state(), State::NeedsAuth);
    assert_eq!(c.queued, Some(("make a cube".into(), false)));
}

#[test]
fn a_plan_update_replaces_the_turns_plan() {
    let mut c = chat();
    c.push(Item::User { text: "go".into(), image: false });
    c.on_event(Event::Plan(vec![("a".into(), "pending".into())]));
    c.on_event(Event::Text { thought: false, text: "working".into() });
    c.on_event(Event::Plan(vec![("a".into(), "completed".into())]));
    let plans: Vec<_> = c.items.iter().filter(|i| matches!(i, Item::Plan(_))).collect();
    assert_eq!(plans.len(), 1);
    assert!(matches!(plans[0], Item::Plan(p) if p[0].1 == "completed"));
}

#[test]
fn a_permission_is_answered_once() {
    let mut c = chat();
    let opts = vec![PermissionOption { id: "allow".into(), name: "Allow".into(), kind: "allow_once".into() }];
    c.on_event(Event::Permission { key: 5, title: "batch".into(), options: opts });
    assert_eq!(c.waiting(), 1);
    c.answer(0, Some("allow"));
    c.answer(0, None);
    assert!(matches!(&c.items[0], Item::Permission { answer: Some(a), .. } if a == "Allow"));
    assert_eq!(c.waiting(), 0);
}

/// An app with the chat's endpoint wired to a channel the test feeds, and a message in progress.
fn app_in_a_turn() -> (SolveApp, Sender<ControlRequest>) {
    let mut app = SolveApp::new(solidcraft_engine::Session::default(), crate::Services::default());
    app.preferences.agent_chat_enabled = true;
    let (tx, rx) = channel();
    app.agent_chat.tools_rx = Some(rx);
    app.agent_chat.state = Some(State::Busy);
    start_turn(&mut app, "two boxes".into(), false);
    (app, tx)
}

fn ask(tx: &Sender<ControlRequest>, method: &str, params: Value) -> Receiver<Value> {
    let (req, rx) = ControlRequest::new(method, params);
    tx.send(req).unwrap();
    rx
}

fn frame(app: &mut SolveApp) {
    let ctx = egui::Context::default();
    poll(app, &ctx);
    app.drain_control(&ctx);
}

fn boxed(x: f64) -> Value {
    json!({"command": "solid.box", "params": {"length": 10, "width": 10, "height": 10, "corner": [x, 0, 0]}})
}

/// Everything the agent changed in one message undoes as one step, and the conversation offers
/// that undo while nothing else has happened since.
#[test]
fn a_messages_changes_undo_as_one_step() {
    let (mut app, tx) = app_in_a_turn();
    let before = app.session.doc.clone();
    let depth = app.session.undo.len();
    let replies: Vec<_> = [0.0, 20.0, 40.0].into_iter().map(|x| ask(&tx, "engine.execute", boxed(x))).collect();
    frame(&mut app);
    for r in replies {
        assert_eq!(r.recv_timeout(std::time::Duration::from_secs(30)).unwrap()["ok"], true);
    }
    assert_eq!(app.session.undo.len(), depth + 3);
    assert_eq!(app.agent_chat.turn.as_ref().map(|t| t.changes), Some(3));
    app.agent_chat.on_event(Event::TurnEnded { reason: String::new() });
    end_turn(&mut app);
    assert_eq!(app.session.undo.len(), depth + 1, "one step");
    assert!(app.session.undo.last().is_some_and(|s| s.label.contains("two boxes")));
    let n = app.agent_chat.items.iter().rposition(|i| matches!(i, Item::Changes { steps: 3, .. })).expect("the changes are listed");
    undo_changes(&mut app, n);
    assert!(Arc::ptr_eq(&app.session.doc, &before), "back to the design before the message");
    assert!(matches!(app.agent_chat.items[n], Item::Changes { undone: true, .. }));
}

/// Saving waits for Allow or Deny; Deny answers the agent without running it, Allow runs it.
#[test]
fn file_actions_wait_for_the_user() {
    let (mut app, tx) = app_in_a_turn();
    let dir = std::env::temp_dir().join(format!("sc-agent-chat-approve-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("a.solidcraft");
    let save = json!({"command": "file.save", "params": {"path": path.to_string_lossy()}, "allow_paths": [dir.to_string_lossy()]});
    let denied = ask(&tx, "engine.execute", save.clone());
    frame(&mut app);
    assert_eq!(app.agent_chat.waiting(), 1);
    assert!(denied.try_recv().is_err(), "nothing answered before the user decides");
    let id = app.agent_chat.held[0].id;
    approve(&mut app, id, false);
    let r = denied.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    assert!(r["error"].as_str().is_some_and(|e| e.contains("declined")), "{r}");
    assert!(!path.exists());
    let allowed = ask(&tx, "engine.execute", save);
    frame(&mut app);
    let id = app.agent_chat.held[0].id;
    approve(&mut app, id, true);
    frame(&mut app);
    let r = allowed.recv_timeout(std::time::Duration::from_secs(30)).unwrap();
    assert_eq!(r["ok"], true, "{r}");
    assert!(path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Crash reports and app methods never run for the agent, requests outside a message don't
/// either, and a message stops taking changes at the limit.
#[test]
fn the_gate_refuses_and_limits() {
    let (mut app, tx) = app_in_a_turn();
    app.preferences.agent_chat_turn_limit = 2;
    let crash = ask(&tx, "engine.execute", json!({"command": "diagnostics.crash_reports", "params": {}}));
    let quit = ask(&tx, "app.quit", json!({"force": true}));
    let ok: Vec<_> = (0..2).map(|i| ask(&tx, "engine.execute", boxed(f64::from(i) * 20.0))).collect();
    let over = ask(&tx, "engine.execute", boxed(80.0));
    let read = ask(&tx, "document.inspect", json!({}));
    frame(&mut app);
    let get = |r: Receiver<Value>| r.recv_timeout(std::time::Duration::from_secs(30)).unwrap();
    assert!(get(crash)["error"].as_str().is_some_and(|e| e.contains("crash reports")));
    assert!(get(quit)["error"].as_str().is_some_and(|e| e.contains("not available")));
    for r in ok {
        assert_eq!(get(r)["ok"], true);
    }
    assert!(get(over)["error"].as_str().is_some_and(|e| e.contains("limit of 2")));
    assert_eq!(get(read)["ok"], true, "reads go on past the limit");
    end_turn(&mut app);
    let late = ask(&tx, "engine.execute", boxed(100.0));
    frame(&mut app);
    assert!(get(late)["error"].as_str().is_some_and(|e| e.contains("no message in progress")));
}

#[test]
fn merging_finds_the_turns_start_when_the_history_was_trimmed() {
    // Distinct designs: what counts is which `Arc` a snapshot holds.
    let doc = |_: usize| Arc::new(solidcraft_engine::doc::Document::default());
    let snap = |d: &Arc<solidcraft_engine::doc::Document>| solidcraft_engine::Snapshot {
        label: String::new(),
        doc: d.clone(),
        active_sketch: None,
        pending_moves: Default::default(),
    };
    let docs: Vec<_> = (0..6).map(doc).collect();
    // The turn began at depth 3 with docs[3]; the history then lost its oldest step.
    let mut undo: Vec<_> = docs[1..6].iter().map(snap).collect();
    assert_eq!(merge_undo(&mut undo, &docs[3], 3, "Agent"), 3);
    assert_eq!(undo.len(), 3);
    assert!(Arc::ptr_eq(&undo[2].doc, &docs[3]) && undo[2].label == "Agent");
    // A start that is no longer in the history: nothing is merged.
    let mut undo: Vec<_> = docs[1..6].iter().map(snap).collect();
    assert_eq!(merge_undo(&mut undo, &doc(9), 3, "Agent"), 0);
    assert_eq!(undo.len(), 5);
}
