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
    h.step(&json!({"click":{"handle":"pdm:advanced"}})).unwrap();
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
    // Reopening inventories saved connections before actions become available.
    h.frames(5);
    assert!(h.app.pdm.runtime.status().unwrap().busy.is_none());
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

use solidcraft_engine::pdm::{ActiveConnection, ConnectionKind, ConnectionStage, ConnectionSummary, ManagedState};

fn guided_recent() -> Vec<ConnectionSummary> {
    vec![
        ConnectionSummary {
            id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            label: "Saved archive label".into(),
            kind: ConnectionKind::Local,
            stage: ConnectionStage::SavedUnverified,
        },
        ConnectionSummary {
            id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            label: "Interrupted archive".into(),
            kind: ConnectionKind::Local,
            stage: ConnectionStage::Pending,
        },
    ]
}
fn guided_worker(requests: Arc<Mutex<Vec<ManagedTask>>>, recent: Vec<ConnectionSummary>) -> Start {
    Arc::new(move |job| {
        let (send, receive) = mpsc::channel();
        let Task::Managed(task) = job.task else {
            send.send(Completion { result: Err(Failure { message: "Unexpected test request".into(), disconnect: false }), open: None }).unwrap();
            return receive;
        };
        requests.lock().unwrap().push(task.clone());
        let result = match task {
            ManagedTask::ListConnections => Ok(Outcome::Connections(recent.clone())),
            ManagedTask::OpenConnection { connection_id } | ManagedTask::RecoverConnection { connection_id } => Ok(Outcome::ManagedConnected {
                port: 48231,
                state: ManagedState {
                    root: Some("C:/PDM proof/Working".into()),
                    server_id: Some("server-real".into()),
                    workspace: Some("verified-workspace".into()),
                    ..Default::default()
                },
                connections: recent.clone(),
                connection: ActiveConnection {
                    id: connection_id,
                    label: "Verified archive".into(),
                    origin: "https://localhost:48231".into(),
                    server_id: "server-real".into(),
                    account: "authenticated-admin".into(),
                    role: "Administrator".into(),
                },
            }),
            ManagedTask::CreateLocal { .. } => Err(Failure { message: "Setup fixture stopped before native work".into(), disconnect: false }),
            _ => Err(Failure { message: "Unexpected guided action".into(), disconnect: false }),
        };
        send.send(Completion { result, open: None }).unwrap();
        receive
    })
}
fn guided_app(recent: Vec<ConnectionSummary>) -> (Harness, Arc<Mutex<Vec<ManagedTask>>>) {
    let mut h = Harness::new();
    let requests = Arc::new(Mutex::new(Vec::new()));
    h.app.services.pdm = Some(guided_worker(requests.clone(), recent));
    h.app.start("pdm.panel");
    h.frames(5);
    (h, requests)
}

#[test]
fn guided_empty_state_lists_once_and_has_no_fabricated_verified_identity() {
    let (mut h, requests) = guided_app(Vec::new());
    assert!(h.widget("Create a local archive to get started.").is_some());
    assert!(h.widget("Create Local Archive").is_some());
    assert!(h.app.pdm.runtime.status().unwrap().connection.is_none());
    h.frames(5);
    assert_eq!(requests.lock().unwrap().iter().filter(|task| matches!(task, ManagedTask::ListConnections)).count(), 1);
    assert!(!requests.lock().unwrap().iter().any(|task| matches!(task, ManagedTask::CreateLocal { .. } | ManagedTask::OpenConnection { .. })));
}

#[test]
fn guided_create_sends_only_public_label_and_administrator_to_worker() {
    let (mut h, requests) = guided_app(Vec::new());
    h.app.pdm.guided.label = "Fixture archive".into();
    h.app.pdm.guided.administrator = "fixture_admin".into();
    h.frames(2);
    h.step(&json!({"click":{"handle":"pdm:setup_create"}})).unwrap();
    h.frames(3);
    assert!(
        requests
            .lock()
            .unwrap()
            .iter()
            .any(|task| matches!(task,ManagedTask::CreateLocal {label,administrator} if label=="Fixture archive" && administrator=="fixture_admin"))
    );
    assert!(h.app.pdm.runtime.status().unwrap().connection.is_none());
    assert!(h.app.session.path.is_none());
}

#[test]
fn guided_existing_open_displays_only_fresh_authenticated_metadata() {
    let (mut h, requests) = guided_app(guided_recent());
    assert!(h.widget("Saved archive label").is_some());
    assert!(h.widget("Not connected").is_some());
    assert!(h.widget("authenticated-admin").is_none());
    assert!(h.app.pdm.runtime.status().unwrap().connection.is_none());
    h.step(&json!({"click":{"handle":"pdm:connection:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}})).unwrap();
    h.frames(4);
    assert!(requests.lock().unwrap().iter().any(|task|matches!(task,ManagedTask::OpenConnection {connection_id} if connection_id=="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")));
    assert!(h.widget("authenticated-admin").is_some());
    assert!(h.widget("Administrator").is_some());
    assert!(h.widget("https://localhost:48231").is_some());
    assert!(h.widget("C:/PDM proof/Working").is_some());
    assert!(h.widget("Connected").is_some());
    assert!(h.widget("Not connected").is_none());
}

#[test]
fn guided_busy_state_disables_setup_and_connection_open() {
    let (mut h, requests) = guided_app(guided_recent());
    h.app.pdm.guided.label = "Fixture archive".into();
    h.frames(2);
    let held = Arc::new(Mutex::new(Vec::<mpsc::Sender<Completion>>::new()));
    let retained = held.clone();
    let observed = requests.clone();
    h.app.services.pdm = Some(Arc::new(move |job| {
        let (send, receive) = mpsc::channel();
        if let Task::Managed(task) = job.task {
            observed.lock().unwrap().push(task);
        }
        retained.lock().unwrap().push(send);
        receive
    }));
    h.app.run("pdm.connections", json!({})).unwrap();
    h.frames(2);
    let before = requests.lock().unwrap().len();
    assert!(h.app.pdm.runtime.status().unwrap().busy.is_some());
    h.step(&json!({"click":{"handle":"pdm:setup_create"}})).unwrap();
    h.step(&json!({"click":{"handle":"pdm:connection:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}})).unwrap();
    h.frames(2);
    assert_eq!(requests.lock().unwrap().len(), before);
    assert!(h.app.pdm.runtime.status().unwrap().busy.is_some());
    assert_eq!(held.lock().unwrap().len(), 1);
}

#[test]
fn guided_pending_row_resumes_existing_installation_without_creating_another() {
    let (mut h, requests) = guided_app(guided_recent());
    assert!(h.widget("Setup incomplete").is_some());
    h.step(&json!({"click":{"handle":"pdm:connection:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}})).unwrap();
    h.frames(4);
    assert!(requests.lock().unwrap().iter().any(|task|matches!(task,ManagedTask::OpenConnection {connection_id} if connection_id=="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")));
    assert!(!requests.lock().unwrap().iter().any(|task| matches!(task, ManagedTask::CreateLocal { .. })));
}

#[test]
fn guided_recovery_requires_confirmation_and_scopes_the_existing_connection() {
    let (mut h, requests) = guided_app(guided_recent());
    let id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    h.step(&json!({"click":{"handle":format!("pdm:connection_recover:{id}")}})).unwrap();
    h.frames(2);
    assert!(h.app.pdm.guided.pending_recovery.is_some());
    assert!(!requests.lock().unwrap().iter().any(|task| matches!(task, ManagedTask::RecoverConnection { .. })));
    h.step(&json!({"click":{"handle":"pdm:connection_recover_confirm"}})).unwrap();
    h.frames(4);
    assert!(requests.lock().unwrap().iter().any(|task| matches!(task,ManagedTask::RecoverConnection {connection_id} if connection_id==id)));
    assert!(!requests.lock().unwrap().iter().any(|task| matches!(task, ManagedTask::CreateLocal { .. })));
}

#[test]
fn guided_recovery_cancel_and_dirty_editor_never_queue_native_recovery() {
    let (mut h, requests) = guided_app(guided_recent());
    let id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    h.step(&json!({"click":{"handle":format!("pdm:connection_recover:{id}")}})).unwrap();
    h.frames(2);
    h.step(&json!({"click":{"handle":"pdm:connection_recover_cancel"}})).unwrap();
    h.frames(2);
    assert!(h.app.pdm.guided.pending_recovery.is_none());
    h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
    h.frames(2);
    h.step(&json!({"click":{"handle":format!("pdm:connection_recover:{id}")}})).unwrap();
    h.frames(2);
    assert!(h.app.pdm.guided.pending_recovery.is_none());
    assert!(!requests.lock().unwrap().iter().any(|task| matches!(task, ManagedTask::RecoverConnection { .. })));
}

#[test]
fn restore_completion_clears_temporary_ram_permission_and_reuses_its_editor() {
    use solidcraft_engine::pdm::{LocalEntry, ManagedState};
    let mut h = app();
    connect(&mut h);
    h.app.session.path = Some("part.solidcraft".into());
    h.app.session.mark_saved();
    let state = |base_version| ManagedState {
        local: vec![LocalEntry {
            document: "part".into(),
            path: "part.solidcraft".into(),
            relative_path: "part.solidcraft".into(),
            base_version,
            sha256: "00".repeat(32),
            checked_out: false,
            modified: false,
            present: true,
        }],
        ..Default::default()
    };
    let id = h.app.pdm.runtime.enqueue(Task::Refresh).unwrap();
    h.app.pdm.runtime.finish(id, Ok(Outcome::Managed { port: 48231, state: state(2) })).unwrap();
    let key = solidcraft_engine::pdm::cached_path_key("part.solidcraft");
    h.app.pdm.ram_paths.insert(key.clone());
    let held = Arc::new(Mutex::new(None));
    h.app.services.pdm = Some(backend(Some(held.clone())));
    let count = h.app.docs.count();
    h.app.run("pdm.restore_version", json!({"document":"part","version":1,"restore":true})).unwrap();
    h.frames(3);
    assert!(h.app.pdm.runtime.status().unwrap().blocking);
    assert!(h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).is_err());
    let mut restored = Session::default();
    restored.path = Some("part.solidcraft".into());
    restored.mark_saved();
    held.lock()
        .unwrap()
        .take()
        .unwrap()
        .send(Completion { result: Ok(Outcome::Managed { port: 48231, state: state(3) }), open: Some(Box::new(restored)) })
        .unwrap();
    h.frames(5);
    assert!(!h.app.pdm.runtime.status().unwrap().blocking);
    assert!(!h.app.pdm.ram_paths.contains(&key));
    assert_eq!(h.app.docs.count(), count);
    assert_eq!(h.app.session.path.as_deref(), Some("part.solidcraft"));
    assert!(!h.app.session.is_dirty());
    assert_eq!(h.app.pdm.runtime.status().unwrap().managed.local.first().unwrap().base_version, 3);
}

#[test]
fn switching_authenticated_connections_clears_old_archive_actions_and_ram_permissions() {
    let (mut h, _) = guided_app(guided_recent());
    let id = h.app.pdm.runtime.enqueue(Task::Connect { port: 48231 }).unwrap();
    h.app
        .pdm
        .runtime
        .finish(
            id,
            Ok(Outcome::Managed {
                port: 48231,
                state: ManagedState { server_id: Some("old-server".into()), workspace: Some("old-workspace".into()), ..Default::default() },
            }),
        )
        .unwrap();
    h.app.pdm.document = "old-document".into();
    h.app.pdm.parent = "9".into();
    h.app.pdm.folder = "Old folder".into();
    h.app.pdm.logical_path = "Old folder/part.solidcraft".into();
    h.app.pdm.selected = Some(Reference { document: "old-document".into(), version: 9 });
    h.app.pdm.dependencies = vec![Reference { document: "old-part".into(), version: 4 }];
    h.app.pdm.restore_confirmation = h.app.pdm.selected.clone();
    h.app.pdm.pending_edit = Some("solid.box".into());
    h.app.pdm.ram_paths.insert("old/part.solidcraft".into());
    h.app.pdm.guided.pending_recovery = Some(("c".repeat(64), "Old connection".into()));
    h.app.run("pdm.connection.open", json!({"connection_id":"a".repeat(64)})).unwrap();
    h.frames(5);
    assert_eq!(h.app.pdm.runtime.status().unwrap().connection.unwrap().account, "authenticated-admin");
    assert!(h.app.pdm.document.is_empty());
    assert!(h.app.pdm.parent.is_empty());
    assert!(h.app.pdm.folder.is_empty());
    assert!(h.app.pdm.logical_path.is_empty());
    assert!(h.app.pdm.selected.is_none());
    assert!(h.app.pdm.dependencies.is_empty());
    assert!(h.app.pdm.restore_confirmation.is_none());
    assert!(h.app.pdm.pending_edit.is_none());
    assert!(h.app.pdm.ram_paths.is_empty());
    assert!(h.app.pdm.guided.pending_recovery.is_none());
}

fn epoch_managed_state(path: &std::path::Path) -> solidcraft_engine::pdm::ManagedState {
    use solidcraft_engine::pdm::{LocalEntry, ManagedState};
    ManagedState {
        server_id: Some("epoch-server".into()),
        workspace: Some("epoch-workspace".into()),
        local: vec![LocalEntry {
            document: "part".into(),
            path: path.to_string_lossy().into_owned(),
            relative_path: "part.solidcraft".into(),
            base_version: 1,
            sha256: "ab".repeat(32),
            checked_out: true,
            modified: false,
            present: true,
        }],
        ..Default::default()
    }
}
fn epoch_connected(h: &mut Harness, path: &std::path::Path) {
    h.app.pdm.runtime.set_available(true).unwrap();
    let request = h.app.pdm.runtime.enqueue(Task::Connect { port: 48231 }).unwrap();
    h.app.pdm.runtime.finish(request, Ok(Outcome::Managed { port: 48231, state: epoch_managed_state(path) })).unwrap();
}
fn epoch_recover(h: &mut Harness, path: &std::path::Path, failed: bool) {
    h.app.pdm.runtime.set_available(true).unwrap();
    let request = h.app.pdm.runtime.enqueue(Task::Managed(ManagedTask::RecoverConnection { connection_id: "a".repeat(64) })).unwrap();
    let mut disk = Session::default();
    disk.execute("solid.box", &json!({"length":7,"width":8,"height":9})).unwrap();
    disk.execute("file.save", &json!({"path":path})).unwrap();
    let result = if failed {
        Err(Failure { message: "Reply lost after disk restoration".into(), disconnect: false })
    } else {
        Ok(Outcome::Managed { port: 48231, state: epoch_managed_state(path) })
    };
    h.app.pdm.runtime.finish(request, result).unwrap();
}
fn epoch_read_fresh(h: &Harness, path: &std::path::Path) -> Session {
    let receipt = h.app.pdm.runtime.begin_pdm_reload().unwrap();
    let mut fresh = Session::default();
    fresh.execute("doc.open", &json!({"path":path})).unwrap();
    crate::documents::bind_fresh_pdm_load(&h.app, &mut fresh, receipt).unwrap();
    fresh
}

#[test]
fn pdm_recovery_epoch_success_and_failure_keep_clean_cached_editor_fenced() {
    for failed in [false, true] {
        let mut h = Harness::new();
        let path = h.dir.join("part.solidcraft");
        h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
        h.app.run("file.save", json!({"path":path})).unwrap();
        epoch_connected(&mut h, &path);
        let old = h.app.session.doc.clone();
        epoch_recover(&mut h, &path, failed);
        let disk = std::fs::read(&path).unwrap();
        assert!(h.app.session.check_pdm_reopen().is_err());
        assert!(h.app.run("file.save", json!({})).is_err());
        h.app.start("solid.box");
        assert!(h.app.dialog.is_none());
        assert!(h.app.pdm.pending_edit.is_none());
        assert!(h.app.status.as_ref().is_some_and(|(text, _, _)| text.contains("Reopen")));
        assert_eq!(*h.app.session.doc, *old);
        let fresh = epoch_read_fresh(&h, &path);
        let expected = fresh.doc.clone();
        crate::documents::apply_managed(&mut h.app, fresh).unwrap();
        assert!(h.app.session.check_pdm_reopen().is_ok());
        assert_eq!(*h.app.session.doc, *expected);
        assert_eq!(std::fs::read(&path).unwrap(), disk);
    }
}

#[test]
fn pdm_recovery_epoch_reopening_one_tab_does_not_bless_another_same_path() {
    let mut h = Harness::new();
    let path = h.dir.join("part.solidcraft");
    h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
    h.app.run("file.save", json!({"path":path})).unwrap();
    epoch_connected(&mut h, &path);
    let original = h.app.docs.active;
    let duplicate = epoch_read_fresh(&h, &path);
    crate::documents::open_tab(&mut h.app, duplicate).unwrap();
    epoch_recover(&mut h, &path, false);
    let fresh = epoch_read_fresh(&h, &path);
    crate::documents::apply_managed(&mut h.app, fresh).unwrap();
    assert!(h.app.session.check_pdm_reopen().is_ok());
    crate::documents::switch(&mut h.app, original);
    assert!(h.app.session.check_pdm_reopen().is_err());
    assert!(h.app.run("file.save", json!({})).is_err());
    h.app.session.mark_unsaved();
    let before = h.app.session.doc.clone();
    let fresh = epoch_read_fresh(&h, &path);
    assert!(crate::documents::apply_managed(&mut h.app, fresh).is_err());
    assert_eq!(*h.app.session.doc, *before);
    assert!(h.app.session.is_dirty());
}

#[test]
fn pdm_recovery_epoch_cached_adoption_never_renews_its_receipt() {
    let mut h = Harness::new();
    let path = h.dir.join("part.solidcraft");
    h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
    h.app.run("file.save", json!({"path":path})).unwrap();
    epoch_connected(&mut h, &path);
    let cached = epoch_read_fresh(&h, &path);
    epoch_recover(&mut h, &path, true);
    crate::documents::adopt(&mut h.app, cached);
    assert!(h.app.session.check_pdm_reopen().is_err());
}
