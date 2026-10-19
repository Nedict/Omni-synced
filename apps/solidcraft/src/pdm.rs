//! Native PDM adapter: all credentials, sockets and private working-copy I/O stay here.
use solidcraft_engine::{
    Session,
    pdm::{Failure, Job, Outcome, Reference, Retrieved, Task, Version},
};
use solidcraft_pdm::{AuthToken, Error, client::Client, workspace::content_path};
use solidcraft_ui_egui::pdm_panel::{Completion, Start};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
};

fn failure(message: &str, disconnect: bool) -> Failure {
    Failure { message: message.into(), disconnect }
}
fn map_error(error: Error) -> Failure {
    match error {
        Error::Server(code) if code == "conflict" => failure("Version conflict. Refresh versions and choose an explicit base.", false),
        Error::Server(code) if code == "unauthorized" => {
            failure("Local PDM authentication failed. Restart the app with the updated local credential.", true)
        }
        Error::Server(code) if code == "not_found" => failure("That exact PDM version is unavailable.", false),
        Error::Integrity => failure("PDM content integrity verification failed.", false),
        Error::Io(_) => failure("Could not read or write the private PDM working files. The connection remains available.", false),
        Error::Network(_) => {
            failure("Local PDM connection failed. A write may have completed; reconnect and refresh versions before retrying.", true)
        }
        Error::Invalid(_) => failure("Local PDM input or credential is unavailable or invalid. Check the local setup instructions.", false),
        _ => failure("Local PDM operation failed. Check the service and private workspace directory.", true),
    }
}
fn versions(rows: Vec<solidcraft_pdm::Version>) -> Result<Vec<Version>, Failure> {
    if rows.len() > 256 {
        return Err(failure("Invalid local PDM version list.", true));
    }
    let rows: Vec<Version> = serde_json::from_value(serde_json::to_value(rows).map_err(|_| failure("Invalid PDM metadata.", true))?)
        .map_err(|_| failure("Invalid PDM metadata.", true))?;
    for row in &rows {
        row.reference.validate().map_err(|_| failure("Invalid PDM reference.", true))?;
        if row.sha256.len() != 64
            || !row.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || row.parent.is_some_and(|parent| parent == 0 || parent >= row.reference.version)
        {
            return Err(failure("Invalid PDM metadata.", true));
        }
        if row.dependencies.len() > 256 {
            return Err(failure("Invalid PDM dependencies.", true));
        }
        for reference in &row.dependencies {
            reference.validate().map_err(|_| failure("Invalid PDM reference.", true))?;
        }
    }
    Ok(rows)
}
fn client(holder: &Mutex<Option<Client>>) -> Result<Client, Failure> {
    holder
        .lock()
        .map_err(|_| failure("Local PDM connection unavailable.", true))?
        .clone()
        .ok_or_else(|| failure("Connect to a local PDM service first.", true))
}

pub fn service(workspace_root: Option<PathBuf>) -> Start {
    let holder = Arc::new(Mutex::new(None::<Client>));
    Arc::new(move |job: Job| {
        let (sender, receiver) = mpsc::channel();
        let holder = holder.clone();
        let root = workspace_root.clone();
        let fallback = sender.clone();
        if std::thread::Builder::new()
            .name("solidcraft-pdm".into())
            .spawn(move || {
                let result = execute(job.task, &holder, root.as_deref());
                let completion = match result {
                    Ok((outcome, open)) => Completion { result: Ok(outcome), open },
                    Err(error) => {
                        if error.disconnect
                            && let Ok(mut held) = holder.lock()
                        {
                            *held = None;
                        }
                        Completion { result: Err(error), open: None }
                    }
                };
                let _ = sender.send(completion);
            })
            .is_err()
        {
            let _ = fallback.send(Completion { result: Err(failure("Could not start the local PDM worker.", true)), open: None });
        }
        receiver
    })
}

fn execute(task: Task, holder: &Mutex<Option<Client>>, workspace_root: Option<&Path>) -> Result<(Outcome, Option<Box<Session>>), Failure> {
    let outcome = match task {
        Task::Connect { port } => {
            // The environment is read only here, never into a command, preferences or a design.
            let connected = Client::new(port, AuthToken::from_env().map_err(map_error)?);
            let rows = versions(connected.list(None).map_err(map_error)?)?;
            *holder.lock().map_err(|_| failure("Local PDM connection unavailable.", true))? = Some(connected);
            Outcome::Connected { port, versions: rows }
        }
        Task::Disconnect => {
            *holder.lock().map_err(|_| failure("Local PDM connection unavailable.", true))? = None;
            Outcome::Disconnected
        }
        Task::Refresh => Outcome::Listed(versions(client(holder)?.list(None).map_err(map_error)?)?),
        Task::CheckIn { document, parent, path, dependencies, saved, visibility } => {
            let bytes = solidcraft_pdm::client::read_file(Path::new(&path), solidcraft_pdm::MAX_CONTENT)
                .map_err(|_| failure("Could not read the saved design. Check its path, file type and 1 MiB size limit.", false))?;
            let mut disk = solidcraft_engine::io::read_design(&bytes).map_err(|_| failure("The saved file is not a readable CAD design.", false))?;
            if disk.visibility != *visibility {
                return Err(failure("The saved file changed outside this design. Reopen or save explicitly before check-in.", false));
            }
            disk.visibility = saved.visibility.clone();
            if disk != *saved {
                return Err(failure("The saved file changed outside this design. Reopen or save explicitly before check-in.", false));
            }
            let dependencies = dependencies.into_iter().map(|r| solidcraft_pdm::VersionRef { document: r.document, version: r.version }).collect();
            let response =
                client(holder)?.request(solidcraft_pdm::Request::CheckIn { document, parent, content: bytes, dependencies }).map_err(map_error)?;
            let returned = match response {
                solidcraft_pdm::Response::CheckedIn { version } => version,
                _ => return Err(failure("PDM check-in returned no version.", true)),
            };
            let version = versions(vec![returned])?.into_iter().next().ok_or_else(|| failure("PDM check-in returned no version.", true))?;
            Outcome::CheckedIn(version)
        }
        Task::Retrieve { reference, open } => {
            let root = workspace_root.ok_or_else(|| failure("Private PDM workspace directory is unavailable.", false))?;
            fs::create_dir_all(root).map_err(|_| failure("Private PDM workspace directory is unavailable.", false))?;
            let mut nonce = [0u8; 16];
            getrandom::fill(&mut nonce).map_err(|_| failure("Could not allocate a new PDM working-copy identifier.", false))?;
            let name: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
            let destination = root.join(format!("copy-{name}"));
            let requested = solidcraft_pdm::VersionRef { document: reference.document.clone(), version: reference.version };
            let snapshot = client(holder)?.retrieve(requested.clone(), &destination).map_err(map_error)?;
            let input = destination.join("snapshot").join(content_path(&requested).map_err(map_error)?);
            let bytes = solidcraft_pdm::client::read_file(&input, solidcraft_pdm::MAX_CONTENT).map_err(map_error)?;
            let copy = destination.join("working.solidcraft");
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&copy)
                .map_err(|_| failure("Could not create the new PDM working copy.", false))?;
            file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|_| failure("Could not write the new PDM working copy.", false))?;
            let path = copy.to_string_lossy().into_owned();
            let retrieved = Retrieved {
                reference: Reference { document: reference.document, version: reference.version },
                path: path.clone(),
                versions: versions(snapshot.versions)?,
            };
            if open {
                let mut session = Session::default();
                session
                    .execute("doc.open", &serde_json::json!({"path":path}))
                    .map_err(|_| failure("The new PDM copy was saved but is not a readable CAD design. The active design is unchanged.", false))?;
                return Ok((Outcome::Retrieved(retrieved), Some(Box::new(session))));
            }
            Outcome::Retrieved(retrieved)
        }
    };
    Ok((outcome, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("solidcraft-pdm-native-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn token() -> AuthToken {
        AuthToken::parse("ab".repeat(32)).unwrap()
    }

    #[test]
    fn malformed_service_metadata_is_rejected_without_echoing_values() {
        let mut row = solidcraft_pdm::Version {
            reference: solidcraft_pdm::VersionRef { document: "part".into(), version: 2 },
            parent: Some(1),
            sha256: "ab".repeat(32),
            dependencies: vec![],
        };
        assert!(versions(vec![row.clone()]).is_ok());
        row.sha256 = "private-invalid-value".into();
        let error = versions(vec![row.clone()]).unwrap_err();
        assert!(error.disconnect);
        assert!(!error.message.contains(&row.sha256));
        row.sha256 = "ab".repeat(32);
        row.parent = Some(2);
        assert!(versions(vec![row]).is_err());
    }
    fn design() -> Session {
        let mut session = Session::default();
        session.execute("solid.box", &serde_json::json!({"length":10,"width":20,"height":3})).unwrap();
        session
    }

    #[test]
    fn changed_saved_file_is_rejected_before_network_or_document_mutation() {
        let temp = Temp::new();
        let path = temp.0.join("saved.solidcraft");
        let mut session = design();
        session.execute("file.save", &serde_json::json!({"path":path})).unwrap();
        let expected = session.doc.clone();
        let other = Session::default();
        fs::write(&path, solidcraft_engine::io::write_design(&other.doc)).unwrap();
        let holder = Mutex::new(Some(Client::new(1, token())));
        let result = execute(
            Task::CheckIn {
                document: "part".into(),
                parent: None,
                path: path.to_string_lossy().into_owned(),
                dependencies: vec![],
                saved: expected.clone(),
                visibility: Box::new(session.visibility.clone()),
            },
            &holder,
            None,
        );
        let failure = match result {
            Err(failure) => failure,
            Ok(_) => panic!("changed disk file was accepted"),
        };
        assert!(failure.message.contains("changed outside"));
        assert!(!failure.disconnect);
        assert_eq!(*session.doc, *expected);
        assert!(!session.is_dirty());
    }

    #[test]
    fn native_saved_design_check_in_and_exact_retrieval_load_a_separate_session() {
        let temp = Temp::new();
        let path = temp.0.join("saved.solidcraft");
        let mut session = design();
        session.execute("file.save", &serde_json::json!({"path":path})).unwrap();
        let expected = session.doc.clone();
        let mut store = solidcraft_pdm::Store::open(&temp.0.join("store")).unwrap();
        let listener = solidcraft_pdm::bind_loopback(0).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (stream, _) = listener.accept().unwrap();
                solidcraft_pdm::serve_connection(&mut store, stream, &token()).unwrap();
            }
        });
        let holder = Mutex::new(Some(Client::new(port, token())));
        let checked = execute(
            Task::CheckIn {
                document: "part".into(),
                parent: None,
                path: path.to_string_lossy().into_owned(),
                dependencies: vec![],
                saved: expected.clone(),
                visibility: Box::new(session.visibility.clone()),
            },
            &holder,
            None,
        )
        .unwrap_or_else(|error| panic!("{}", error.message));
        assert!(matches!(checked.0, Outcome::CheckedIn(_)));
        let workspace = temp.0.join("workspaces");
        let (outcome, loaded) =
            execute(Task::Retrieve { reference: Reference { document: "part".into(), version: 1 }, open: true }, &holder, Some(&workspace))
                .unwrap_or_else(|error| panic!("{}", error.message));
        assert!(matches!(outcome, Outcome::Retrieved(_)));
        let loaded = loaded.expect("separate loaded design");
        assert_eq!(*loaded.doc, *expected);
        assert_eq!(*session.doc, *expected);
        assert!(!session.is_dirty());
        assert!(!loaded.is_dirty());
        let copy = PathBuf::from(loaded.path.as_deref().unwrap());
        assert!(copy.starts_with(&workspace));
        assert!(copy.ends_with("working.solidcraft"));
        assert_ne!(copy, path);
        assert_eq!(fs::read(copy).unwrap(), fs::read(path).unwrap());
        server.join().unwrap();
    }
}
