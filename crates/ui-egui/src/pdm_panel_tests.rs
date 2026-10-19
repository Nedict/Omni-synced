use crate::{
    pdm_panel::{Completion, Start},
    scenario::Harness,
};
use serde_json::json;
use solidcraft_engine::{
    Session,
    pdm::{Failure, ManagedTask, Outcome, Reference, Retrieved, Task, Version},
};
use std::sync::{Arc, Mutex, mpsc};

fn version() -> Version {
    Version {
        reference: Reference { document: "part".into(), version: 1 },
        parent: None,
        sha256: "00".repeat(32),
        dependencies: vec![Reference { document: "base".into(), version: 3 }],
    }
}
fn backend(held: Option<Arc<Mutex<Option<mpsc::Sender<Completion>>>>>) -> Start {
    Arc::new(move |job| {
        let (send, receive) = mpsc::channel();
        if let Some(held) = &held {
            *held.lock().unwrap() = Some(send);
            return receive;
        }
        let (outcome, open) = match job.task {
            Task::Managed(ManagedTask::Connect { port, .. }) => (Outcome::Connected { port, versions: vec![version()] }, None),
            Task::Managed(ManagedTask::History { .. }) => (Outcome::Listed(vec![version()]), None),
            Task::Managed(_) => (Outcome::Listed(vec![version()]), None),
            Task::Connect { port } => (Outcome::Connected { port, versions: vec![version()] }, None),
            Task::Disconnect => (Outcome::Disconnected, None),
            Task::Refresh => (Outcome::Managed { port: 48231, state: Default::default() }, None),
            Task::CheckIn { .. } => (Outcome::CheckedIn(Version { reference: Reference { document: "part".into(), version: 2 }, ..version() }), None),
            Task::Retrieve { reference, open } => {
                let mut session = Session::default();
                session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap();
                session.mark_saved();
                session.path = Some("new-private-copy.solidcraft".into());
                (
                    Outcome::Retrieved(Retrieved { reference, path: "new-private-copy.solidcraft".into(), versions: vec![version()] }),
                    open.then(|| Box::new(session)),
                )
            }
        };
        send.send(Completion { result: Ok(outcome), open }).unwrap();
        receive
    })
}
fn app() -> Harness {
    let mut h = Harness::new();
    h.app.services.pdm = Some(backend(None));
    h.app.start("pdm.panel");
    h.frames(3);
    h
}
fn connect(h: &mut Harness) {
    h.step(&json!({"click":{"handle":"pdm:connect"}})).unwrap();
    h.frames(5);
    h.app.pdm.open = false;
    h.app.pdm.document = "part".into();
    h.frames(2);
    assert_eq!(h.app.pdm.runtime.status().unwrap().port, Some(48231));
}

#[test]
fn actual_panel_connects_lists_pinned_dependencies_and_disconnects() {
    let mut h = app();
    connect(&mut h);
    h.step(&json!({"click":{"handle":"pdm:version:part:1"}})).unwrap();
    h.frames(2);
    assert_eq!(h.app.pdm.selected, Some(Reference { document: "part".into(), version: 1 }));
    h.app.pdm.detail_tab = 1;
    h.frames(2);
    assert!(h.widget("base · v3").is_some());
    h.app.pdm.open = true;
    h.frames(2);
    h.step(&json!({"click":{"handle":"pdm:disconnect"}})).unwrap();
    h.frames(4);
    assert!(h.app.pdm.runtime.status().unwrap().port.is_none());
}

#[test]
fn retrieval_opens_a_new_tab_and_preserves_dirty_active_design_and_undo() {
    let mut h = app();
    connect(&mut h);
    h.app.run("solid.box", json!({"length":10,"width":20,"height":3})).unwrap();
    let before = h.app.session.doc.clone();
    let undo = h.app.session.undo.len();
    let active = h.app.docs.active;
    assert!(h.app.session.is_dirty());
    h.app.run("pdm.retrieve", json!({"document":"part","version":1})).unwrap();
    h.frames(5);
    assert_eq!(h.app.session.path.as_deref(), Some("new-private-copy.solidcraft"));
    assert!(!h.app.session.is_dirty());
    crate::documents::switch(&mut h.app, active);
    h.frames(2);
    assert_eq!(*h.app.session.doc, *before);
    assert_eq!(h.app.session.undo.len(), undo);
    assert!(h.app.session.is_dirty());
}

#[test]
fn full_tab_limit_never_replaces_dirty_design_on_retrieval() {
    let mut h = app();
    connect(&mut h);
    for _ in 1..crate::documents::MAX_OPEN {
        crate::documents::open_tab(&mut h.app, Session::default()).unwrap();
    }
    h.app.run("solid.box", json!({"length":10,"width":20,"height":3})).unwrap();
    let before = h.app.session.doc.clone();
    h.app.run("pdm.retrieve", json!({"document":"part","version":1})).unwrap();
    h.frames(5);
    assert_eq!(*h.app.session.doc, *before);
    assert!(h.app.session.is_dirty());
    assert_eq!(h.app.docs.count(), crate::documents::MAX_OPEN);
    assert!(h.app.pdm.runtime.status().unwrap().retrieved.is_some());
}

#[test]
fn completed_retrieval_waits_for_a_modelling_dialog_started_during_download() {
    let mut h = app();
    connect(&mut h);
    h.app.run("solid.box", json!({"length":10,"width":20,"height":3})).unwrap();
    let before = h.app.session.doc.clone();
    let active = h.app.docs.active;
    let held = Arc::new(Mutex::new(None));
    h.app.services.pdm = Some(backend(Some(held.clone())));
    h.app.run("pdm.retrieve", json!({"document":"part","version":1})).unwrap();
    h.frames(2);
    h.app.start("solid.box");
    assert!(h.app.dialog.is_some());
    h.app.start("pdm.panel");
    assert!(h.app.dialog.is_some());
    let mut loaded = Session::default();
    loaded.path = Some("new-private-copy.solidcraft".into());
    held.lock()
        .unwrap()
        .take()
        .unwrap()
        .send(Completion {
            result: Ok(Outcome::Retrieved(Retrieved {
                reference: version().reference,
                path: "new-private-copy.solidcraft".into(),
                versions: vec![version()],
            })),
            open: Some(Box::new(loaded)),
        })
        .unwrap();
    h.frames(4);
    assert_eq!(h.app.docs.active, active);
    assert!(h.app.dialog.is_some());
    assert!(h.app.pdm.runtime.status().unwrap().busy.is_some());
    assert_eq!(*h.app.session.doc, *before);
    crate::dialogs::cancel(&mut h.app);
    h.frames(4);
    assert_eq!(h.app.session.path.as_deref(), Some("new-private-copy.solidcraft"));
    crate::documents::switch(&mut h.app, active);
    assert_eq!(*h.app.session.doc, *before);
    assert!(h.app.session.is_dirty());
}

#[test]
fn saved_check_in_uses_engine_guards_and_async_failure_keeps_the_document() {
    let mut h = app();
    connect(&mut h);
    h.app.run("solid.box", json!({"length":10,"width":20,"height":3})).unwrap();
    assert!(h.app.run("pdm.check_in", json!({"document":"part"})).is_err());
    h.app.run("file.save", json!({"path":h.dir.join("saved.solidcraft")})).unwrap();
    h.app.pdm.document = "part".into();
    h.app.pdm.parent = "1".into();
    h.frames(2);
    h.app.run("pdm.check_in", json!({"document":"part","parent":1})).unwrap();
    h.frames(5);
    assert_eq!(h.app.pdm.parent, "2");
    let before = h.app.session.doc.clone();
    let held = Arc::new(Mutex::new(None));
    h.app.services.pdm = Some(backend(Some(held.clone())));
    h.app.run("pdm.refresh", json!({})).unwrap();
    h.frames(2);
    assert!(h.app.run("pdm.disconnect", json!({})).is_err());
    held.lock()
        .unwrap()
        .take()
        .unwrap()
        .send(Completion { result: Err(Failure { message: "Local service unavailable".into(), disconnect: true }), open: None })
        .unwrap();
    h.frames(4);
    assert!(h.app.pdm.runtime.status().unwrap().port.is_none());
    assert_eq!(*h.app.session.doc, *before);
    assert!(!h.app.session.is_dirty());
    let prefs = serde_json::to_string(&h.app.prefs()).unwrap();
    assert!(!prefs.contains("SOLIDCRAFT_PDM_TOKEN"));
    assert!(!prefs.contains("pdm"));
}

#[test]
fn stale_worker_completion_cannot_open_a_tab_or_change_the_active_design() {
    let mut h = app();
    connect(&mut h);
    h.app.run("solid.box", json!({"length":10,"width":20,"height":3})).unwrap();
    let before = h.app.session.doc.clone();
    let count = h.app.docs.count();
    let held = Arc::new(Mutex::new(None));
    h.app.services.pdm = Some(backend(Some(held.clone())));
    h.app.run("pdm.retrieve", json!({"document":"part","version":1})).unwrap();
    h.frames(2);
    let id = h.app.pdm.runtime.status().unwrap().busy.unwrap();
    h.app.pdm.runtime.finish(id, Ok(Outcome::Disconnected)).unwrap();
    held.lock()
        .unwrap()
        .take()
        .unwrap()
        .send(Completion {
            result: Ok(Outcome::Retrieved(Retrieved { reference: version().reference, path: "stale-copy.solidcraft".into(), versions: vec![] })),
            open: Some(Box::new(Session::default())),
        })
        .unwrap();
    h.frames(4);
    assert_eq!(h.app.docs.count(), count);
    assert_eq!(*h.app.session.doc, *before);
    assert!(h.app.session.is_dirty());
    assert!(h.app.pdm.runtime.status().unwrap().retrieved.is_none());
}

#[test]
fn managed_reload_reuses_the_existing_tab_and_refuses_dirty_work() {
    let mut h = Harness::new();
    h.app.session.path = Some("managed.solidcraft".into());
    h.app.session.mark_saved();
    let count = h.app.docs.count();
    let mut replacement = Session::default();
    replacement.path = Some("managed.solidcraft".into());
    replacement.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap();
    replacement.mark_saved();
    crate::documents::apply_managed(&mut h.app, replacement).unwrap();
    assert_eq!(h.app.docs.count(), count);
    assert_eq!(h.app.session.world_state().bodies.len(), 1);
    h.app.session.execute("solid.box", &json!({"length":5,"width":6,"height":7})).unwrap();
    let before = h.app.session.doc.clone();
    let mut replacement = Session::default();
    replacement.path = Some("managed.solidcraft".into());
    replacement.mark_saved();
    assert!(crate::documents::apply_managed(&mut h.app, replacement).is_err());
    assert_eq!(*h.app.session.doc, *before);
    assert!(h.app.session.is_dirty());
}

#[test]
fn managed_readonly_interactive_edit_prompts_before_starting_a_tool() {
    use solidcraft_engine::pdm::{LocalEntry, ManagedState};
    let mut h = Harness::new();
    h.app.pdm.runtime.set_available(true).unwrap();
    h.app.session.path = Some("managed.solidcraft".into());
    h.app.session.mark_saved();
    let id = h.app.pdm.runtime.enqueue(Task::Connect { port: 48231 }).unwrap();
    h.app
        .pdm
        .runtime
        .finish(
            id,
            Ok(Outcome::Managed {
                port: 48231,
                state: ManagedState {
                    local: vec![LocalEntry {
                        document: "part".into(),
                        path: "managed.solidcraft".into(),
                        relative_path: "managed.solidcraft".into(),
                        base_version: 1,
                        sha256: "00".repeat(32),
                        checked_out: false,
                        modified: false,
                        present: true,
                    }],
                    ..Default::default()
                },
            }),
        )
        .unwrap();
    h.app.start("solid.box");
    assert_eq!(h.app.pdm.pending_edit.as_deref(), Some("solid.box"));
    assert!(h.app.dialog.is_none());
    assert!(!h.app.session.is_dirty());
    assert!(h.app.pdm.runtime.may_save("managed.solidcraft").is_err());
}

#[test]
fn historical_snapshot_edit_prompts_and_cannot_be_checked_out_in_place() {
    let mut h = Harness::new();
    h.app.session.path = Some("history/snapshot.solidcraft".into());
    h.app.session.pdm_read_only = true;
    h.app.session.mark_saved();
    h.app.start("solid.box");
    assert_eq!(h.app.pdm.pending_edit.as_deref(), Some("solid.box"));
    assert!(h.app.dialog.is_none());
}

#[test]
fn workspace_switcher_opens_docked_archive_and_preserves_the_design() {
    let mut h = Harness::new();
    h.app.run("solid.box", json!({"length":10,"width":20,"height":3})).unwrap();
    let before = h.app.session.doc.clone();
    let undo = h.app.session.undo.len();
    h.frames(3);
    let at = crate::scenario::rect_of("workspace_switcher").unwrap().center();
    h.step(&json!({"click":[at.x,at.y]})).unwrap();
    h.step(&json!({"click":{"handle":"menu:ui.workspace.pdm"}})).unwrap();
    assert!(h.app.pdm.workspace_active);
    assert!(!h.app.pdm.open, "the main browser is docked, not a floating window");
    assert!(h.widget("Archive folders").is_some());
    assert!(crate::scenario::rect_of("toolbar_tab:PDM").is_some());
    let at = crate::scenario::rect_of("workspace_switcher").unwrap().center();
    h.step(&json!({"click":[at.x,at.y]})).unwrap();
    h.step(&json!({"click":{"handle":"menu:ui.workspace.design"}})).unwrap();
    assert!(!h.app.pdm.workspace_active);
    assert_eq!(*h.app.session.doc, *before);
    assert_eq!(h.app.session.undo.len(), undo);
}

#[test]
fn local_recovery_requires_a_root_and_explicit_confirmation() {
    let mut h = app();
    h.step(&json!({"click":{"handle":"pdm:recover_workspace"}})).unwrap();
    assert!(!h.app.pdm.recovery_confirmation);
    h.app.pdm.root = h.dir.to_string_lossy().to_string();
    h.frames(2);
    h.step(&json!({"click":{"handle":"pdm:recover_workspace"}})).unwrap();
    assert!(h.app.pdm.recovery_confirmation);
    assert!(h.app.pdm.runtime.status().unwrap().busy.is_none(), "opening confirmation does not queue recovery");
    h.step(&json!({"widget":"Cancel"})).unwrap();
    assert!(!h.app.pdm.recovery_confirmation);
    assert!(h.app.pdm.runtime.status().unwrap().busy.is_none());
}

#[cfg(target_os = "windows")]
#[test]
fn extended_windows_managed_path_reuses_tab_and_detects_unsaved_editor() {
    use solidcraft_engine::pdm::{LocalEntry, ManagedState};
    let mut h = Harness::new();
    h.app.session.path = Some(r"C:\Work\part.solidcraft".into());
    h.app.session.mark_saved();
    let count = h.app.docs.count();
    let mut replacement = Session::default();
    replacement.path = Some(r"\\?\c:\work\part.solidcraft".into());
    replacement.mark_saved();
    crate::documents::apply_managed(&mut h.app, replacement).unwrap();
    assert_eq!(h.app.docs.count(), count);
    h.app.session.path = Some(r"C:\Work\part.solidcraft".into());
    h.app.session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap();
    h.app.pdm.runtime.set_available(true).unwrap();
    let id = h.app.pdm.runtime.enqueue(Task::Connect { port: 48231 }).unwrap();
    h.app
        .pdm
        .runtime
        .finish(
            id,
            Ok(Outcome::Managed {
                port: 48231,
                state: ManagedState {
                    local: vec![LocalEntry {
                        document: "part".into(),
                        path: r"\\?\c:\work\part.solidcraft".into(),
                        relative_path: "part.solidcraft".into(),
                        base_version: 1,
                        sha256: "00".repeat(32),
                        checked_out: true,
                        modified: false,
                        present: true,
                    }],
                    ..Default::default()
                },
            }),
        )
        .unwrap();
    assert!(crate::documents::managed_target_dirty(&h.app, Some("part")));
    assert!(h.app.run("pdm.get_latest", json!({"document":"part"})).is_err());
}

#[test]
fn metadata_refresh_keeps_explicit_temporary_ram_edit_permission() {
    let mut h = app();
    connect(&mut h);
    let key = solidcraft_engine::pdm::cached_path_key("managed.solidcraft");
    h.app.pdm.ram_paths.insert(key.clone());
    h.app.run("pdm.refresh", json!({})).unwrap();
    h.frames(5);
    assert!(h.app.pdm.ram_paths.contains(&key));
}

#[test]
fn restore_version_confirmation_captures_selection_and_cancel_queues_nothing() {
    use solidcraft_engine::pdm::{LocalEntry, ManagedState};
    let mut h = app();
    connect(&mut h);
    let id = h.app.pdm.runtime.enqueue(Task::Refresh).unwrap();
    h.app
        .pdm
        .runtime
        .finish(
            id,
            Ok(Outcome::Managed {
                port: 48231,
                state: ManagedState {
                    local: vec![LocalEntry {
                        document: "part".into(),
                        path: "part.solidcraft".into(),
                        relative_path: "part.solidcraft".into(),
                        base_version: 2,
                        sha256: "00".repeat(32),
                        checked_out: false,
                        modified: false,
                        present: true,
                    }],
                    ..Default::default()
                },
            }),
        )
        .unwrap();
    h.app.pdm.selected = Some(version().reference);
    h.frames(2);
    h.step(&json!({"click":{"handle":"pdm:restore_version"}})).unwrap();
    assert_eq!(h.app.pdm.restore_confirmation, Some(version().reference));
    assert!(h.app.pdm.runtime.status().unwrap().busy.is_none());
    h.step(&json!({"widget":"Cancel"})).unwrap();
    assert!(h.app.pdm.restore_confirmation.is_none());
    assert!(h.app.pdm.runtime.status().unwrap().busy.is_none());

    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    h.app.services.pdm = Some(Arc::new(move |job| {
        captured.lock().unwrap().push(job.task);
        let (_send, receive) = mpsc::channel();
        receive
    }));
    h.step(&json!({"click":{"handle":"pdm:restore_version"}})).unwrap();
    h.app.pdm.selected = Some(Reference { document: "different".into(), version: 9 });
    h.step(&json!({"click":{"handle":"pdm:restore_version_confirm"}})).unwrap();
    h.frames(2);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(matches!(&requests[0], Task::Managed(ManagedTask::Restore { reference }) if *reference == version().reference));
    assert!(h.app.pdm.restore_confirmation.is_none());
}
