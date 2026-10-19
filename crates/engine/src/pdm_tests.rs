use super::*;
use crate::Session;
use serde_json::json;

#[test]
fn first_archive_blocks_concurrent_edits_and_save_before_manifest_registration() {
    let mut session = connected();
    let folder = std::env::temp_dir().join(format!("solidcraft-first-archive-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("first.solidcraft");
    session.execute("file.save", &json!({"path":path})).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let request = session.execute("pdm.archive", &json!({})).unwrap()["request"].as_u64().unwrap();
    assert!(session.pdm.managed_for_path(path.to_str().unwrap()).is_none());
    assert!(session.pdm.blocks_path(path.to_str().unwrap()));
    assert!(session.execute("file.save", &json!({})).is_err());
    assert!(session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    session.pdm.finish(request, Err(Failure { message: "offline".into(), disconnect: false })).unwrap();
    assert!(!session.pdm.blocks_path(path.to_str().unwrap()));
    session.execute("file.save", &json!({})).unwrap();
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn managed_save_guard_is_independent_of_os_readonly_and_allows_a_separate_copy() {
    let mut session = Session::default();
    let folder = std::env::temp_dir().join(format!("solidcraft-managed-save-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("managed.solidcraft");
    session.execute("file.save", &json!({"path":path})).unwrap();
    session.pdm.set_available(true).unwrap();
    let id = session.pdm.enqueue(Task::Connect { port: 48231 }).unwrap();
    session
        .pdm
        .finish(
            id,
            Ok(Outcome::Managed {
                port: 48231,
                state: ManagedState {
                    local: vec![LocalEntry {
                        document: "doc".into(),
                        path: path.to_string_lossy().into_owned(),
                        relative_path: "managed.solidcraft".into(),
                        base_version: 1,
                        sha256: "ab".repeat(32),
                        checked_out: false,
                        modified: false,
                        present: true,
                    }],
                    ..ManagedState::default()
                },
            }),
        )
        .unwrap();
    let bytes = std::fs::read(&path).unwrap();
    session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap();
    assert!(session.execute("file.save", &json!({})).is_err());
    assert!(session.execute("file.save_as", &json!({"path":folder.join(".").join("managed.solidcraft")})).is_err());
    assert!(session.execute("file.export", &json!({"path":folder.join(".").join("managed.solidcraft"),"format":"design"})).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    session.execute("file.save_as", &json!({"path":folder.join("copy.solidcraft")})).unwrap();
    session.execute("file.save", &json!({})).unwrap();
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn historical_guard_survives_edits_but_save_as_detaches_the_copy() {
    let mut session = Session::default();
    let folder = std::env::temp_dir().join(format!("solidcraft-history-save-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    session.execute("file.save", &json!({"path":folder.join("history.solidcraft")})).unwrap();
    session.pdm_read_only = true;
    session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap();
    assert!(session.execute("file.save", &json!({})).is_err());
    assert!(session.execute("file.save_as", &json!({"path":folder.join(".").join("history.solidcraft")})).is_err());
    session.execute("file.save_as", &json!({"path":folder.join("copy.solidcraft")})).unwrap();
    assert!(!session.pdm_read_only);
    session.execute("file.save", &json!({})).unwrap();
    std::fs::remove_dir_all(folder).unwrap();
}

fn connected() -> Session {
    let mut session = Session::default();
    session.pdm.set_available(true).unwrap();
    let request = session.execute("pdm.connect", &json!({"port":48231})).unwrap()["request"].as_u64().unwrap();
    assert!(matches!(session.pdm.take_job().unwrap().unwrap().task, Task::Connect { port: 48231 }));
    session.pdm.finish(request, Ok(Outcome::Connected { port: 48231, versions: vec![] })).unwrap();
    session
}

#[test]
fn host_availability_ports_and_single_job_guards_are_enforced_without_design_edits() {
    let mut session = Session::default();
    assert!(session.execute("pdm.connect", &json!({"port":48231})).is_err());
    session.pdm.set_available(true).unwrap();
    for port in [json!(0), json!(65536), json!(1.5), json!("bad")] {
        assert!(session.execute("pdm.connect", &json!({"port":port})).is_err());
    }
    let doc = session.doc.clone();
    let revision = session.revision;
    let id = session.execute("pdm.connect", &json!({"port":48231})).unwrap()["request"].as_u64().unwrap();
    assert!(session.execute("pdm.connect", &json!({"port":48232})).is_err());
    assert!(session.execute("pdm.disconnect", &json!({})).is_err());
    assert!(session.pdm.finish(id + 1, Ok(Outcome::Disconnected)).is_err());
    assert_eq!(session.pdm.status().unwrap().busy, Some(id));
    session.pdm.finish(id, Err(Failure { message: "Service unavailable".into(), disconnect: true })).unwrap();
    assert!(session.pdm.status().unwrap().port.is_none());
    assert_eq!(*session.doc, *doc);
    assert_eq!(session.revision, revision);
    assert!(session.undo.is_empty());
    assert!(!session.is_dirty());
}

#[test]
fn check_in_requires_saved_unchanged_design_and_retains_explicit_base_and_dependencies() {
    let mut session = connected();
    assert!(session.execute("pdm.check_in", &json!({"document":"part"})).is_err());
    session.path = Some("saved.solidcraft".into());
    session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap();
    assert!(session.execute("pdm.check_in", &json!({"document":"part"})).is_err());
    session.mark_saved();
    session.execute("pdm.check_in", &json!({"document":"assembly","parent":4,"dependencies":[{"document":"part","version":2}]})).unwrap();
    let job = session.pdm.take_job().unwrap().unwrap();
    match job.task {
        Task::CheckIn { document, parent, path, dependencies, saved, .. } => {
            assert_eq!(document, "assembly");
            assert_eq!(parent, Some(4));
            assert_eq!(path, "saved.solidcraft");
            assert_eq!(dependencies, vec![Reference { document: "part".into(), version: 2 }]);
            assert_eq!(*saved, *session.doc);
        }
        other => panic!("unexpected task {other:?}"),
    }
    session.pdm.finish(job.id, Err(Failure { message: "Version conflict".into(), disconnect: false })).unwrap();
    assert_eq!(session.pdm.status().unwrap().port, Some(48231));
    assert!(session.pdm.status().unwrap().error.is_some());
    assert!(!session.is_dirty());
}

#[test]
fn retrieval_validates_refs_and_only_requests_a_new_working_copy() {
    let mut session = connected();
    let before = session.doc.clone();
    for document in ["../escape", "UPPER", "a/b", ""] {
        assert!(session.execute("pdm.retrieve", &json!({"document":document,"version":1})).is_err());
    }
    assert!(session.execute("pdm.retrieve", &json!({"document":"part","version":0})).is_err());
    session.execute("pdm.retrieve", &json!({"document":"part","version":1})).unwrap();
    assert!(matches!(session.pdm.take_job().unwrap().unwrap().task, Task::Retrieve { reference: Reference { version: 1, .. }, open: true }));
    assert_eq!(*session.doc, *before);
    assert!(session.path.is_none());
}

#[test]
fn active_edits_and_bad_parent_or_dependency_metadata_are_refused() {
    let mut session = connected();
    session.path = Some("saved.solidcraft".into());
    session.active_sketch = Some(7);
    assert!(session.execute("pdm.check_in", &json!({"document":"part"})).is_err());
    session.active_sketch = None;
    for params in [
        json!({"document":"part","parent":0}),
        json!({"document":"part","parent":"bad"}),
        json!({"document":"part","dependencies":[{"document":"../escape","version":1}]}),
    ] {
        assert!(session.execute("pdm.check_in", &params).is_err());
    }
    assert!(session.pdm.take_job().unwrap().is_none());
}

#[test]
fn credentials_and_unknown_parameters_are_refused_before_command_recording() {
    let mut session = connected();
    let secret = "credential-must-not-be-recorded";
    for (command, params) in [
        ("pdm.connect", json!({"port":48231,"token":secret})),
        ("pdm.status", json!({"credential":secret})),
        ("pdm.check_in", json!({"document":"part","dependencies":[{"document":"base","version":1,"token":secret}]})),
    ] {
        let error = session.execute(command, &params).unwrap_err().to_string();
        assert!(!error.contains(secret));
        assert!(session.pdm.take_job().unwrap().is_none());
    }
    assert!(!serde_json::to_string(&session.pdm.status().unwrap()).unwrap().contains(secret));
    assert!(!session.scratch().pdm.status().unwrap().available);
}
