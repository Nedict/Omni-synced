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
    assert!(session.execute("assembly.bom", &json!({"path":path,"format":"json"})).is_err());
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
    assert!(session.execute("file.export", &json!({"path":path,"format":"urdf"})).is_err());
    assert!(session.execute("assembly.bom", &json!({"path":folder.join(".").join("managed.solidcraft"),"format":"json"})).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    session.execute("assembly.bom", &json!({"path":folder.join("parts.csv")})).unwrap();
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
    let bytes = std::fs::read(folder.join("history.solidcraft")).unwrap();
    session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap();
    assert!(session.execute("file.save", &json!({})).is_err());
    assert!(session.execute("file.save_as", &json!({"path":folder.join(".").join("history.solidcraft")})).is_err());
    assert!(session.execute("assembly.bom", &json!({"path":folder.join(".").join("history.solidcraft"),"format":"json"})).is_err());
    assert_eq!(std::fs::read(folder.join("history.solidcraft")).unwrap(), bytes);
    session.execute("assembly.bom", &json!({"path":folder.join("historical-parts.csv")})).unwrap();
    assert!(session.pdm_read_only);
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

fn managed_snapshot(path: &str, workspace: &str, checked_out: bool) -> ManagedState {
    ManagedState {
        server_id: Some("server".into()),
        workspace: Some(workspace.into()),
        local: vec![LocalEntry {
            document: "part".into(),
            path: path.into(),
            relative_path: "part.solidcraft".into(),
            base_version: 1,
            sha256: "ab".repeat(32),
            checked_out,
            modified: false,
            present: true,
        }],
        ..ManagedState::default()
    }
}

fn apply_managed(runtime: &Runtime, managed: ManagedState, refresh: bool) -> Result<(), String> {
    let id = runtime.enqueue(Task::Connect { port: 48231 })?;
    runtime.finish(
        id,
        Ok(if refresh { Outcome::ManagedRefreshed { port: 48231, state: managed } } else { Outcome::Managed { port: 48231, state: managed } }),
    )
}

#[test]
fn inactive_workspace_paths_stay_protected_and_verified_reconnection_restores_checkout() {
    let mut session = connected();
    let folder = std::env::temp_dir().join(format!("solidcraft-workspace-switch-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("part.solidcraft");
    session.execute("file.save", &json!({"path":path})).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let path = path.to_str().unwrap();
    let inactive_tab = session.pdm.clone();
    apply_managed(&session.pdm, managed_snapshot(path, "a", false), false).unwrap();
    assert!(inactive_tab.may_save(path).is_err());
    let id = session.pdm.enqueue(Task::Disconnect).unwrap();
    session.pdm.finish(id, Ok(Outcome::Disconnected)).unwrap();
    assert!(inactive_tab.may_save(path).is_err());
    for refresh in [false, true] {
        apply_managed(&session.pdm, managed_snapshot("another.solidcraft", "b", false), refresh).unwrap();
        assert!(!inactive_tab.managed_for_path(path).unwrap().checked_out);
        assert!(session.execute("file.save", &json!({})).is_err());
        let alias = folder.join(".").join("part.solidcraft");
        assert!(session.execute("file.save_as", &json!({"path":alias})).is_err());
        assert!(session.execute("file.export", &json!({"path":alias,"format":"design"})).is_err());
        assert!(session.execute("assembly.bom", &json!({"path":alias,"format":"json"})).is_err());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    apply_managed(&session.pdm, managed_snapshot(path, "a", true), false).unwrap();
    session.pdm.may_save(path).unwrap();
    // Switching away preserves the server reservation, but requires verified
    // reconnection before this application's inactive workspace can save again.
    apply_managed(&session.pdm, managed_snapshot("another.solidcraft", "b", false), false).unwrap();
    assert!(inactive_tab.may_save(path).is_err());
    apply_managed(&session.pdm, managed_snapshot(path, "a", true), true).unwrap();
    assert!(inactive_tab.managed_for_path(path).unwrap().checked_out);
    session.execute("file.save", &json!({})).unwrap();
    apply_managed(&session.pdm, managed_snapshot(path, "a", false), true).unwrap();
    assert!(inactive_tab.may_save(path).is_err());
    session.execute("file.save_as", &json!({"path":folder.join("separate.solidcraft")})).unwrap();
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn historical_destinations_remain_readonly_for_other_tabs_and_workspaces() {
    let runtime = connected().pdm;
    let mut snapshot = managed_snapshot("working.solidcraft", "a", true);
    snapshot.historical_paths = vec!["snapshot/root.solidcraft".into(), "snapshot/component.solidcraft".into()];
    apply_managed(&runtime, snapshot, false).unwrap();
    runtime.may_save("working.solidcraft").unwrap();
    apply_managed(&runtime, managed_snapshot("other.solidcraft", "b", false), true).unwrap();
    for path in ["snapshot/root.solidcraft", "snapshot/component.solidcraft"] {
        assert!(runtime.may_save(path).is_err());
    }
    runtime.may_save("independent.solidcraft").unwrap();
}

#[test]
fn conflicting_workspace_path_and_protection_overflow_fail_closed() {
    let runtime = connected().pdm;
    apply_managed(&runtime, managed_snapshot("part.solidcraft", "a", false), false).unwrap();
    assert!(apply_managed(&runtime, managed_snapshot("part.solidcraft", "b", true), false).is_err());
    assert!(runtime.status().unwrap().busy.is_none());
    assert!(runtime.status().unwrap().error.is_some());
    assert!(runtime.may_save("part.solidcraft").is_err());
    assert!(runtime.enqueue(Task::Connect { port: 48231 }).is_err());
    let runtime = connected().pdm;
    let snapshot = ManagedState {
        historical_paths: (0..MAX_PROTECTED_PATHS + 1).map(|n| format!("snapshot/{n}.solidcraft")).collect(),
        ..ManagedState::default()
    };
    assert!(apply_managed(&runtime, snapshot, false).is_err());
    assert!(runtime.may_save("snapshot/0.solidcraft").is_err());
    assert!(runtime.may_save("unregistered.solidcraft").is_err());
}

#[test]
fn historical_ledger_retains_original_paths_for_save_time_resolution() {
    let runtime = connected().pdm;
    let path = r"\\?\UNC\server\share\history\part.solidcraft";
    let snapshot = ManagedState { historical_paths: vec![path.into()], ..ManagedState::default() };
    apply_managed(&runtime, snapshot, false).unwrap();
    assert_eq!(runtime.0.lock().unwrap().historical_paths.get(&cached_path_key(path)).unwrap(), path);
    if cfg!(target_os = "windows") {
        assert_eq!(cached_path_key(path), cached_path_key(r"\\server\share\history\part.solidcraft"));
        assert_eq!(cached_path_key(r"\\?\unc\server\share\history\part.solidcraft"), cached_path_key(path));
    }
}

#[test]
fn urdf_export_checks_every_mesh_before_writing_any_target() {
    let mut session = connected();
    for name in ["A", "B"] {
        session.execute("component.activate", &json!({"component":"root"})).unwrap();
        session.execute("component.create", &json!({"name":name})).unwrap();
        session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap();
    }
    let folder = std::env::temp_dir().join(format!("solidcraft-pdm-urdf-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let package =
        solidcraft_io::urdf(&session.doc, &session.doc.painted(&session.model.state()), "robot", &solidcraft_io::UrdfOptions::default()).unwrap();
    assert!(package.meshes.len() >= 2);
    let first = folder.join(&package.meshes.first().unwrap().0);
    let last = folder.join(&package.meshes.last().unwrap().0);
    std::fs::write(&first, b"existing first mesh").unwrap();
    std::fs::write(&last, b"protected last mesh").unwrap();
    let destination = folder.join("robot.urdf");
    apply_managed(&session.pdm, managed_snapshot(last.to_str().unwrap(), "a", false), false).unwrap();
    for historical in [false, true] {
        if historical {
            session.pdm = Runtime::default();
            session.pdm_read_only = true;
            session.path = Some(last.to_string_lossy().into_owned());
        }
        let error = session.execute("file.export", &json!({"path":destination,"format":"urdf"})).unwrap_err().to_string();
        assert!(error.contains(if historical { "Historical copies are read-only" } else { "Extract this managed design" }), "{error}");
        assert_eq!(std::fs::read(&first).unwrap(), b"existing first mesh");
        assert_eq!(std::fs::read(&last).unwrap(), b"protected last mesh");
        assert!(!destination.exists());
    }
    let separate = folder.join("separate");
    std::fs::create_dir(&separate).unwrap();
    session.execute("file.export", &json!({"path":separate.join("robot.urdf"),"format":"urdf"})).unwrap();
    assert!(separate.join("robot.urdf").exists());
    assert_eq!(std::fs::read(&last).unwrap(), b"protected last mesh");
    std::fs::remove_dir_all(folder).unwrap();
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

#[test]
fn restore_in_flight_blocks_edits_and_outputs_in_every_editor_for_its_workspace() {
    let mut session = connected();
    let folder = std::env::temp_dir().join(format!("solidcraft-pdm-restore-inflight-{}", std::process::id()));
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("part.solidcraft");
    session.execute("file.save", &json!({"path":path})).unwrap();
    let before = std::fs::read(&path).unwrap();
    apply_managed(&session.pdm, managed_snapshot(path.to_str().unwrap(), "a", false), false).unwrap();
    let mut second_editor = Session { path: Some(path.to_string_lossy().into_owned()), pdm: session.pdm.clone(), ..Default::default() };
    second_editor.mark_saved();
    let id = session.execute("pdm.restore_version", &json!({"document":"part","version":1,"restore":true})).unwrap()["request"].as_u64().unwrap();
    assert!(session.pdm.status().unwrap().blocking);
    for editor in [&mut session, &mut second_editor] {
        let doc = editor.doc.clone();
        let error = editor.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap_err().to_string();
        assert!(error.contains("Wait for the managed operation"));
        assert_eq!(*editor.doc, *doc);
        assert!(editor.execute("file.save", &json!({})).is_err());
        assert!(editor.execute("parameters.export", &json!({"path":path,"format":"json"})).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    session.pdm.finish(id, Err(Failure { message: "Restore did not change the working copy".into(), disconnect: false })).unwrap();
    assert!(!session.pdm.status().unwrap().blocking);
    second_editor.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_dir_all(folder).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hardlink_destinations_cannot_truncate_historical_or_inactive_managed_files() {
    let folder = std::env::temp_dir().join(format!("solidcraft-pdm-hardlinks-{}", std::process::id()));
    std::fs::create_dir(&folder).unwrap();
    let original = folder.join("protected.solidcraft");
    let alias = folder.join("report.json");
    let bytes = b"protected original bytes";
    std::fs::write(&original, bytes).unwrap();
    std::fs::hard_link(&original, &alias).unwrap();
    assert_ne!(std::fs::canonicalize(&original).unwrap(), std::fs::canonicalize(&alias).unwrap());
    assert!(same_file_path(original.to_str().unwrap(), alias.to_str().unwrap()));
    for protection in 0..3 {
        let mut session = connected();
        match protection {
            0 => {
                let state = ManagedState { historical_paths: vec![original.to_string_lossy().into_owned()], ..ManagedState::default() };
                apply_managed(&session.pdm, state, false).unwrap();
                apply_managed(&session.pdm, managed_snapshot("other.solidcraft", "b", true), false).unwrap();
            }
            1 => {
                apply_managed(&session.pdm, managed_snapshot(original.to_str().unwrap(), "a", false), false).unwrap();
                apply_managed(&session.pdm, managed_snapshot("other.solidcraft", "b", true), false).unwrap();
            }
            _ => {
                session.pdm_read_only = true;
                session.path = Some(original.to_string_lossy().into_owned());
            }
        }
        for (command, params) in [
            ("parameters.export", json!({"path":alias,"format":"json"})),
            ("timeline.export_script", json!({"path":alias})),
            ("assembly.bom", json!({"path":alias,"format":"json"})),
            ("file.save_as", json!({"path":alias})),
        ] {
            let error = session.execute(command, &params).unwrap_err().to_string();
            assert!(
                error.contains(if protection == 1 { "Extract this managed design" } else { "Historical copies are read-only" }),
                "{command}: {error}"
            );
            assert_eq!(std::fs::read(&original).unwrap(), bytes);
            assert_eq!(std::fs::read(&alias).unwrap(), bytes);
        }
        session.execute("parameters.export", &json!({"path":folder.join(format!("separate-{protection}.json")),"format":"json"})).unwrap();
        assert_eq!(std::fs::read(&original).unwrap(), bytes);
    }
    std::fs::remove_dir_all(folder).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn busy_managed_destinations_block_existing_hardlinks_until_completion() {
    let folder = std::env::temp_dir().join(format!("solidcraft-pdm-busy-hardlinks-{}", std::process::id()));
    std::fs::create_dir(&folder).unwrap();
    let original = folder.join("working.solidcraft");
    let alias = folder.join("alias.solidcraft");
    std::fs::write(&original, b"unchanged").unwrap();
    std::fs::hard_link(&original, &alias).unwrap();
    let runtime = connected().pdm;
    apply_managed(&runtime, managed_snapshot(original.to_str().unwrap(), "a", true), false).unwrap();
    runtime.may_save(alias.to_str().unwrap()).unwrap();
    let id = runtime.enqueue(Task::Managed(ManagedTask::GetLatest { document: Some("part".into()), path: None })).unwrap();
    assert!(runtime.may_save(alias.to_str().unwrap()).unwrap_err().contains("Wait for the managed operation"));
    assert_eq!(std::fs::read(&original).unwrap(), b"unchanged");
    runtime.finish(id, Err(Failure { message: "No local replacement".into(), disconnect: false })).unwrap();
    runtime.may_save(alias.to_str().unwrap()).unwrap();
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn guided_public_requests_validate_before_queue_and_preserve_the_design() {
    let mut session = Session::default();
    session.pdm.set_available(true).unwrap();
    let before = session.doc.clone();
    let secret = "do-not-record-credentials";
    for (command, parameters) in [
        ("pdm.setup.create", json!({"label":"Archive","administrator":"admin","token":secret})),
        ("pdm.setup.create", json!({"label":"Archive\nOther","administrator":"admin"})),
        ("pdm.setup.create", json!({"label":"Archive","administrator":"owner"})),
        ("pdm.setup.create", json!({"label":"Archive","administrator":"ADMIN"})),
        ("pdm.connection.open", json!({"connection_id":"../escape"})),
        ("pdm.connection.open", json!({"connection_id":"A".repeat(64)})),
        ("pdm.connection.recover", json!({"connection_id":"a".repeat(64)})),
        ("pdm.connections", json!({"credential":secret})),
    ] {
        let error = session.execute(command, &parameters).unwrap_err().to_string();
        assert!(!error.contains(secret));
        assert!(session.pdm.status().unwrap().busy.is_none());
        assert!(session.pdm.take_job().unwrap().is_none());
        assert_eq!(*session.doc, *before);
        assert!(session.undo.is_empty());
    }
    session.execute("pdm.setup.create", &json!({"label":"Archive","administrator":"admin"})).unwrap();
    assert!(
        matches!(session.pdm.take_job().unwrap().unwrap().task, Task::Managed(ManagedTask::CreateLocal {label,administrator}) if label=="Archive" && administrator=="admin")
    );
    assert_eq!(*session.doc, *before);
}

#[test]
fn saved_inventory_never_establishes_or_replaces_authenticated_identity() {
    let runtime = Runtime::default();
    runtime.set_available(true).unwrap();
    let rows = vec![ConnectionSummary {
        id: "a".repeat(64),
        label: "Unverified label".into(),
        kind: ConnectionKind::Saved,
        stage: ConnectionStage::SavedUnverified,
    }];
    let id = runtime.enqueue(Task::Managed(ManagedTask::ListConnections)).unwrap();
    runtime.finish(id, Ok(Outcome::Connections(rows.clone()))).unwrap();
    assert!(runtime.status().unwrap().connection.is_none());
    assert!(runtime.status().unwrap().port.is_none());
    let active = ActiveConnection {
        id: "a".repeat(64),
        label: "Verified archive".into(),
        origin: "https://localhost:48231".into(),
        server_id: "server".into(),
        account: "actual-account".into(),
        role: "Viewer".into(),
    };
    let id = runtime.enqueue(Task::Managed(ManagedTask::OpenConnection { connection_id: active.id.clone() })).unwrap();
    runtime
        .finish(
            id,
            Ok(Outcome::ManagedConnected {
                port: 48231,
                state: managed_snapshot("managed.solidcraft", "a", false),
                connections: rows.clone(),
                connection: active.clone(),
            }),
        )
        .unwrap();
    let id = runtime.enqueue(Task::Managed(ManagedTask::ListConnections)).unwrap();
    runtime.finish(id, Ok(Outcome::Connections(rows))).unwrap();
    assert_eq!(runtime.status().unwrap().connection, Some(active.clone()));
    let id = runtime.enqueue(Task::Managed(ManagedTask::OpenConnection { connection_id: "b".repeat(64) })).unwrap();
    runtime.finish(id, Err(Failure { message: "Authentication failed".into(), disconnect: false })).unwrap();
    assert_eq!(runtime.status().unwrap().connection, Some(active));
    apply_managed(&runtime, managed_snapshot("other.solidcraft", "b", false), true).unwrap();
    assert!(runtime.status().unwrap().connection.is_none(), "a new workspace snapshot must not inherit verified identity");
    assert!(runtime.may_save("managed.solidcraft").is_err());
}

#[test]
fn scoped_recovery_rejects_unsaved_active_work_before_queueing() {
    let mut session = Session::default();
    session.pdm.set_available(true).unwrap();
    session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap();
    let before = session.doc.clone();
    assert!(session.execute("pdm.connection.recover", &json!({"connection_id":"a".repeat(64),"restore":true})).is_err());
    assert!(session.pdm.take_job().unwrap().is_none());
    assert_eq!(*session.doc, *before);
}

#[test]
fn opaque_connection_recovery_blocks_saved_editors_until_native_completion() {
    let mut session = Session::default();
    session.pdm.set_available(true).unwrap();
    session.path = Some("not-yet-discovered-workspace/part.solidcraft".into());
    session.mark_saved();
    let before = session.doc.clone();
    let id = session.execute("pdm.connection.recover", &json!({"connection_id":"a".repeat(64),"restore":true})).unwrap()["request"].as_u64().unwrap();
    assert!(session.pdm.blocks_path(session.path.as_deref().unwrap()));
    assert!(session.pdm.may_save("another-destination.json").is_err());
    assert!(session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).is_err());
    assert_eq!(*session.doc, *before);
    session.pdm.finish(id, Err(Failure { message: "Recovery root unavailable; no files changed".into(), disconnect: false })).unwrap();
    assert!(!session.pdm.blocks_path(session.path.as_deref().unwrap()));
    session.pdm.may_save("another-destination.json").unwrap();
    assert!(session.execute("solid.box", &json!({"length":2,"width":3,"height":4})).unwrap_err().to_string().contains("Reopen"));
    assert_eq!(*session.doc, *before);
}

struct EpochTemp(std::path::PathBuf);
impl EpochTemp {
    fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("solidcraft-recovery-epoch-{label}-{}-{sequence}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for EpochTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn epoch_editor(path: &std::path::Path) -> Session {
    let mut session = connected();
    session.execute("solid.box", &json!({"length":10,"width":20,"height":3})).unwrap();
    session.execute("file.save", &json!({"path":path})).unwrap();
    let request = session.pdm.enqueue(Task::Refresh).unwrap();
    session
        .pdm
        .finish(request, Ok(Outcome::ManagedRefreshed { port: 48231, state: managed_snapshot(path.to_str().unwrap(), "workspace", true) }))
        .unwrap();
    session
}

#[test]
fn recovery_success_failure_and_refresh_never_authorize_stale_clean_editor() {
    for guided in [false, true] {
        for success in [false, true] {
            let temp = EpochTemp::new("outcomes");
            let path = temp.0.join("part.solidcraft");
            let mut editor = epoch_editor(&path);
            assert!(!editor.is_dirty());
            let old_doc = editor.doc.clone();
            let task = if guided {
                ManagedTask::RecoverConnection { connection_id: "a".repeat(64) }
            } else {
                ManagedTask::Recover { port: 48231, root: temp.0.to_string_lossy().into_owned() }
            };
            let request = editor.pdm.enqueue(Task::Managed(task)).unwrap();
            assert_eq!(editor.pdm.recovery_epoch().unwrap(), 1);
            assert!(editor.pdm.begin_pdm_reload().is_err());
            // Native recovery replaces bytes while the clean editor still holds B.
            let recovered = Session::default();
            let recovered_bytes = solidcraft_io::write_design(&recovered.doc);
            std::fs::write(&path, &recovered_bytes).unwrap();
            let state = managed_snapshot(path.to_str().unwrap(), "workspace", true);
            let result = if success {
                Ok(Outcome::ManagedRefreshed { port: 48231, state })
            } else {
                Err(Failure { message: "recovery interrupted".into(), disconnect: false })
            };
            editor.pdm.finish(request, result).unwrap();
            let refresh = editor.pdm.enqueue(Task::Refresh).unwrap();
            editor
                .pdm
                .finish(refresh, Ok(Outcome::ManagedRefreshed { port: 48231, state: managed_snapshot(path.to_str().unwrap(), "workspace", true) }))
                .unwrap();
            assert_eq!(editor.pdm.recovery_epoch().unwrap(), 1);
            assert!(editor.pdm.managed_for_path(path.to_str().unwrap()).unwrap().checked_out);
            assert!(editor.execute("solid.box", &json!({"length":1,"width":2,"height":3})).is_err());
            assert!(editor.execute("file.save", &json!({})).is_err());
            assert!(editor.execute("assembly.bom", &json!({"path":path,"format":"json"})).is_err());
            assert!(editor.execute("pdm.archive", &json!({})).is_err());
            assert!(editor.execute("pdm.check_in", &json!({"document":"part"})).is_err());
            assert_eq!(*editor.doc, *old_doc);
            assert_eq!(std::fs::read(&path).unwrap(), recovered_bytes);
            assert!(!editor.is_dirty());
            assert!(editor.pdm.status().unwrap().busy.is_none());
            assert!(editor.pdm.take_job().unwrap().is_none());
            editor.execute("doc.open", &json!({"path":path})).unwrap();
            assert!(editor.doc.features.is_empty(), "verified reopen must load recovered A, not retain stale B");
            assert!(editor.check_pdm_reopen().is_ok());
        }
    }
}

#[test]
fn verified_reload_refreshes_only_that_editor_not_another_tab_or_copy() {
    let temp = EpochTemp::new("two-editors");
    let path = temp.0.join("part.solidcraft");
    let mut first = epoch_editor(&path);
    let mut second = Session { pdm: first.pdm.clone(), ..Default::default() };
    second.execute("doc.open", &json!({"path":path})).unwrap();
    let request = first.pdm.enqueue(Task::Managed(ManagedTask::RecoverConnection { connection_id: "a".repeat(64) })).unwrap();
    first.pdm.finish(request, Err(Failure { message: "recovery failed".into(), disconnect: false })).unwrap();
    first.execute("doc.open", &json!({"path":path})).unwrap();
    assert_eq!(first.pdm_recovery_epoch, 1);
    assert!(first.check_pdm_reopen().is_ok());
    assert!(second.check_pdm_reopen().is_err());
    let before = std::fs::read(&path).unwrap();
    let old_epoch = second.pdm_recovery_epoch;
    assert!(second.execute("file.save_as", &json!({"path":temp.0.join("missing").join("copy.solidcraft")})).is_err());
    assert_eq!(second.pdm_recovery_epoch, old_epoch);
    assert_eq!(second.path.as_deref(), path.to_str());
    let copy = temp.0.join("copy.solidcraft");
    second.execute("file.save_as", &json!({"path":copy})).unwrap();
    assert_eq!(second.pdm_recovery_epoch, 1);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(second.check_pdm_reopen().is_ok());
    // A third cached editor remains stale even after both other editors proceed.
    let mut third = Session { path: Some(path.to_string_lossy().into_owned()), pdm: first.pdm.clone(), ..Default::default() };
    assert!(third.execute("file.save", &json!({})).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn recovery_between_read_start_and_stamp_cannot_authorize_cached_bytes() {
    let temp = EpochTemp::new("async-race");
    let path = temp.0.join("part.solidcraft");
    let mut editor = epoch_editor(&path);
    let captured = editor.pdm.begin_pdm_reload().unwrap();
    let request = editor.pdm.enqueue(Task::Managed(ManagedTask::RecoverConnection { connection_id: "a".repeat(64) })).unwrap();
    assert!(editor.mark_pdm_reloaded_from(captured).is_err());
    editor.pdm.finish(request, Err(Failure { message: "failed".into(), disconnect: false })).unwrap();
    assert!(editor.mark_pdm_reloaded_from(captured).is_err());
    assert_eq!(editor.pdm_recovery_epoch, 0);
    assert!(editor.check_pdm_reopen().is_err());
    editor.execute("doc.open", &json!({"path":path})).unwrap();
    assert_eq!(editor.pdm_recovery_epoch, 1);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn stale_editor_cannot_export_over_a_native_hardlink_alias() {
    let temp = EpochTemp::new("hardlink");
    let path = temp.0.join("part.solidcraft");
    let alias = temp.0.join("alias.solidcraft");
    let mut editor = epoch_editor(&path);
    std::fs::hard_link(&path, &alias).unwrap();
    let before = std::fs::read(&path).unwrap();
    let request = editor.pdm.enqueue(Task::Managed(ManagedTask::RecoverConnection { connection_id: "a".repeat(64) })).unwrap();
    editor.pdm.finish(request, Err(Failure { message: "failed".into(), disconnect: false })).unwrap();
    assert!(editor.execute("file.save", &json!({"path":alias})).is_err());
    assert!(editor.execute("assembly.bom", &json!({"path":alias,"format":"json"})).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read(&alias).unwrap(), before);
}

#[test]
fn unnamed_editor_is_editable_and_stamps_epoch_only_after_new_file_success() {
    let temp = EpochTemp::new("unnamed");
    let mut editor = connected();
    let request = editor.pdm.enqueue(Task::Managed(ManagedTask::RecoverConnection { connection_id: "a".repeat(64) })).unwrap();
    editor.pdm.finish(request, Err(Failure { message: "failed".into(), disconnect: false })).unwrap();
    editor.execute("solid.box", &json!({"length":1,"width":2,"height":3})).unwrap();
    assert_eq!(editor.pdm_recovery_epoch, 0);
    assert!(editor.execute("file.save", &json!({"path":temp.0.join("missing").join("first.solidcraft")})).is_err());
    assert_eq!(editor.pdm_recovery_epoch, 0);
    assert!(editor.path.is_none());
    editor.execute("file.save", &json!({"path":temp.0.join("first.solidcraft")})).unwrap();
    assert_eq!(editor.pdm_recovery_epoch, 1);
    assert!(editor.check_pdm_reopen().is_ok());
}

#[test]
fn recovery_epoch_exhaustion_and_poison_fail_closed_for_saved_editors() {
    let temp = EpochTemp::new("failed-protection");
    let path = temp.0.join("part.solidcraft");
    let mut editor = epoch_editor(&path);
    editor.pdm.0.lock().unwrap().recovery_epoch = u64::MAX;
    editor.pdm_recovery_epoch = u64::MAX;
    let bytes = std::fs::read(&path).unwrap();
    assert!(editor.pdm.enqueue(Task::Managed(ManagedTask::RecoverConnection { connection_id: "a".repeat(64) })).is_err());
    assert_eq!(editor.pdm.0.lock().unwrap().recovery_epoch, u64::MAX);
    assert!(editor.execute("file.save", &json!({})).is_err());
    assert!(editor.execute("solid.box", &json!({"length":1,"width":2,"height":3})).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let runtime = Runtime::default();
    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _state = runtime.0.lock().unwrap();
        panic!("test-only PDM runtime poisoning");
    }));
    assert!(poisoned.is_err());
    assert!(runtime.requires_reopen(0, Some(path.to_str().unwrap())));
    assert!(runtime.begin_pdm_reload().is_err());
}
