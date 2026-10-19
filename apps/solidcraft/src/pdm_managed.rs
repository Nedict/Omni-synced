//! Native managed workspace adapter. Reservation capabilities never cross into UI state.
use solidcraft_engine::{
    Session,
    pdm::{CatalogEntry, Failure, LocalEntry, ManagedState, ManagedTask, Outcome, Reference},
};
use solidcraft_pdm::{
    AuthToken, Error, VersionRef,
    client::{Client, read_file},
    managed::{CheckoutGrant, ManagedCheckIn},
    managed_workspace::Workspace,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub struct Host {
    client: Option<Client>,
    port: u16,
    root: Option<PathBuf>,
    server_id: String,
    machine: String,
    grants: BTreeMap<String, CheckoutGrant>,
}
fn fail(message: &str) -> Failure {
    Failure { message: message.into(), disconnect: false }
}
fn error(error: Error) -> Failure {
    super::pdm::map_error(error)
}
fn convert<T: serde::de::DeserializeOwned>(value: impl serde::Serialize) -> Result<T, Failure> {
    serde_json::from_value(serde_json::to_value(value).map_err(|_| fail("Invalid managed metadata"))?).map_err(|_| fail("Invalid managed metadata"))
}
fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

impl Host {
    pub fn connected(&self) -> bool {
        self.client.is_some()
    }
    pub fn disconnect(&mut self) {
        self.client = None;
    }
    pub(crate) fn client(&self) -> Result<Client, Failure> {
        self.client.clone().ok_or_else(|| fail("Connect to a local PDM service first"))
    }
    fn workspace(&self) -> Result<Workspace, Failure> {
        Workspace::open(self.root.as_deref().ok_or_else(|| fail("Choose a working folder first"))?, &self.server_id).map_err(error)
    }
    fn state(&self, ws: &Workspace, selected: Option<&str>) -> Result<ManagedState, Failure> {
        let client = self.client()?;
        let catalog = client.managed_catalog().map_err(error)?;
        if catalog.server_id != self.server_id {
            return Err(fail("The service identity changed; reconnect explicitly"));
        }
        let mut local = Vec::new();
        for (document, entry) in &ws.manifest().documents {
            let path = ws.path(document).map_err(error)?;
            let actual = ws.local_sha256(document);
            local.push(LocalEntry {
                document: document.clone(),
                path: path.to_string_lossy().into_owned(),
                relative_path: entry.relative_path.clone(),
                base_version: entry.base_version,
                sha256: entry.base_sha256.clone(),
                checked_out: entry.checked_out
                    && self.grants.get(document).is_some_and(|grant| {
                        catalog
                            .entries
                            .iter()
                            .any(|remote| remote.document == *document && remote.checkout.is_some() && remote.checkout == grant.entry.checkout)
                    }),
                modified: actual.as_ref().is_ok_and(|hash| hash != &entry.base_sha256),
                present: actual.is_ok(),
            });
        }
        let mut state = ManagedState {
            server_id: Some(self.server_id.clone()),
            root: Some(ws.root().to_string_lossy().into_owned()),
            workspace: Some(ws.manifest().workspace_id.clone()),
            catalog: convert::<Vec<CatalogEntry>>(catalog.entries)?,
            local,
            history: vec![],
            where_used: vec![],
            preview: None,
            historical_paths: vec![],
        };
        if let Some(document) = selected {
            state.history = convert(client.managed_history(document.into()).map_err(error)?)?;
            if let Some(latest) = state.history.last() {
                state.preview = self.preview(&client, &latest.version.reference)?;
                let response = client
                    .request(solidcraft_pdm::Request::WhereUsed {
                        reference: VersionRef { document: document.into(), version: latest.version.reference.version },
                    })
                    .map_err(error)?;
                if let solidcraft_pdm::Response::WhereUsed { versions } = response {
                    state.where_used = convert::<Vec<Reference>>(versions)?;
                }
            }
        }
        Ok(state)
    }
    pub fn refresh(&self) -> Result<(Outcome, Option<Box<Session>>), Failure> {
        let ws = self.workspace()?;
        Ok((Outcome::ManagedRefreshed { port: self.port, state: self.state(&ws, None)? }, None))
    }
    fn preview(&self, client: &Client, reference: &Reference) -> Result<Option<solidcraft_engine::pdm::Preview>, Failure> {
        let (version, bytes) = client.download(&VersionRef { document: reference.document.clone(), version: reference.version }).map_err(error)?;
        Ok(solidcraft_engine::io::read_design_thumbnail(&bytes).map(|png| solidcraft_engine::pdm::Preview {
            reference: reference.clone(),
            sha256: version.sha256,
            png,
        }))
    }
    fn target(&self, ws: &Workspace, document: Option<String>, path: Option<String>) -> Result<String, Failure> {
        if let Some(document) = document {
            return Ok(document);
        }
        let wanted = fs::canonicalize(path.ok_or_else(|| fail("Select an archived file"))?).map_err(|_| fail("The active file is unavailable"))?;
        for document in ws.manifest().documents.keys() {
            if fs::canonicalize(ws.path(document).map_err(error)?).ok().as_ref() == Some(&wanted) {
                return Ok(document.clone());
            }
        }
        Err(fail("The active design is not managed in this working folder"))
    }
    pub fn execute(&mut self, task: ManagedTask, default_root: Option<&Path>) -> Result<(Outcome, Option<Box<Session>>), Failure> {
        if let ManagedTask::Recover { port, root } = &task {
            let client = Client::new(*port, AuthToken::from_env().map_err(error)?);
            let catalog = client.managed_catalog().map_err(error)?;
            let path = Path::new(root);
            if !path.is_absolute() {
                return Err(fail("Recovery requires an absolute workspace path"));
            }
            let mut workspace = Workspace::open_for_recovery(path, &catalog.server_id).map_err(error)?;
            for (document, entry) in &workspace.manifest().documents {
                let (version, _) = client.download(&VersionRef { document: document.clone(), version: entry.base_version }).map_err(error)?;
                if version.sha256 != entry.base_sha256 {
                    return Err(fail("Recovery refused: the recorded base does not match this server"));
                }
            }
            workspace.recover_pending(true).map_err(error)?;
            drop(workspace);
            return self.execute(ManagedTask::Connect { port: *port, root: Some(root.clone()) }, default_root);
        }
        if let ManagedTask::Connect { port, root } = task {
            let client = Client::new(port, AuthToken::from_env().map_err(error)?);
            let catalog = client.managed_catalog().map_err(error)?;
            let root = root
                .map(PathBuf::from)
                .or_else(|| default_root.map(|p| p.join("working")))
                .ok_or_else(|| fail("Choose an absolute working folder"))?;
            if !root.is_absolute() {
                return Err(fail("The working folder must be absolute"));
            }
            if let Some(parent) = root.parent() {
                fs::create_dir_all(parent).map_err(|_| fail("Could not create working folder parent"))?;
            }
            let ws = if root.join(".solidcraft-pdm").exists() {
                Workspace::open(&root, &catalog.server_id)
            } else {
                Workspace::create(&root, &catalog.server_id)
            }
            .map_err(error)?;
            self.port = port;
            self.root = Some(ws.root().into());
            self.server_id = catalog.server_id;
            self.machine = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "desktop".into());
            self.client = Some(client.clone());
            self.grants.clear();
            // Reacquire only reservations that already belong to this persisted workspace.
            for entry in client.managed_catalog().map_err(error)?.entries {
                if entry.checkout.as_ref().is_some_and(|c| c.workspace == ws.manifest().workspace_id && c.machine == self.machine)
                    && let Ok(grant) = client.managed_checkout(entry.document.clone(), ws.manifest().workspace_id.clone(), self.machine.clone())
                {
                    self.grants.insert(entry.document, grant);
                }
            }
            return Ok((Outcome::Managed { port, state: self.state(&ws, None)? }, None));
        }
        let client = self.client()?;
        if client.managed_catalog().map_err(error)?.server_id != self.server_id {
            return Err(fail("The service identity changed; reconnect before modifying this workspace"));
        }
        let mut ws = self.workspace()?;
        let mut opened = None;
        let selected = match task {
            ManagedTask::Recover { .. } => return Err(fail("Invalid recovery state")),
            ManagedTask::Preview { reference } => {
                let mut state = self.state(&ws, Some(&reference.document))?;
                state.preview = self.preview(&client, &reference)?;
                let response = client
                    .request(solidcraft_pdm::Request::WhereUsed {
                        reference: VersionRef { document: reference.document, version: reference.version },
                    })
                    .map_err(error)?;
                if let solidcraft_pdm::Response::WhereUsed { versions } = response {
                    state.where_used = convert(versions)?;
                }
                return Ok((Outcome::Managed { port: self.port, state }, None));
            }
            ManagedTask::Restore { reference } => {
                let local = ws
                    .manifest()
                    .documents
                    .get(&reference.document)
                    .cloned()
                    .ok_or_else(|| fail("Retrieve this document into the working folder before restoring history"))?;
                if local.checked_out || ws.local_sha256(&reference.document).map_err(error)? != local.base_sha256 {
                    return Err(fail("Restore refused: the local copy is modified or checked out"));
                }
                let requested = VersionRef { document: reference.document.clone(), version: reference.version };
                let (source, bytes) = client.download(&requested).map_err(error)?;
                if source.reference != requested || hash(&bytes) != source.sha256 {
                    return Err(fail("Historical content integrity verification failed"));
                }
                solidcraft_engine::io::read_design(&bytes).map_err(|_| fail("The historical version is not a readable native design"))?;
                let history = client.managed_history(reference.document.clone()).map_err(error)?;
                let receipt = restore_receipt(&ws.manifest().workspace_id, &reference, local.base_version, &source.sha256, &source.dependencies)?;
                // Check receipts before acquiring a grant. This covers a lost publication reply
                // and retries after the local manifest has already advanced to the restored head.
                let published = history.into_iter().find(|version| {
                    if version.version.sha256 != source.sha256 || version.version.dependencies != source.dependencies {
                        return false;
                    }
                    version.request_id == receipt
                        || (version.version.reference.version == local.base_version
                            && version.version.sha256 == local.base_sha256
                            && version.version.parent.is_some_and(|parent| {
                                restore_receipt(&ws.manifest().workspace_id, &reference, parent, &source.sha256, &source.dependencies)
                                    .is_ok_and(|id| id == version.request_id)
                            }))
                });
                let remote = client
                    .managed_catalog()
                    .map_err(error)?
                    .entries
                    .into_iter()
                    .find(|entry| entry.document == reference.document)
                    .ok_or_else(|| fail("The archived document is missing"))?;
                let published = match published {
                    Some(version) => {
                        if remote.head != Some(version.version.reference.version) {
                            return Err(fail("Restore was already published, but a newer head exists; update the working copy first"));
                        }
                        version
                    }
                    None => {
                        if remote.head != Some(local.base_version) {
                            return Err(fail("Restore refused: update the clean working copy to the current head first"));
                        }
                        let grant = client
                            .managed_checkout(reference.document.clone(), ws.manifest().workspace_id.clone(), self.machine.clone())
                            .map_err(error)?;
                        self.grants.insert(reference.document.clone(), grant.clone());
                        if grant.entry.head != Some(local.base_version) {
                            return Err(fail("The archive changed before restore; your checkout is preserved for reconciliation"));
                        }
                        client
                            .managed_check_in(ManagedCheckIn {
                                document: reference.document.clone(),
                                workspace: ws.manifest().workspace_id.clone(),
                                machine: self.machine.clone(),
                                checkout_token: grant.token,
                                request_id: receipt,
                                parent: Some(local.base_version),
                                content: bytes.clone(),
                                dependencies: source.dependencies.clone(),
                                comment: format!("Restore archived version {}", reference.version),
                            })
                            .map_err(error)?
                    }
                };
                if local.base_version != published.version.reference.version {
                    ws.install(&reference.document, &local.relative_path, published.version.reference.version, &published.version.sha256, &bytes)
                        .map_err(error)?;
                }
                self.grants.remove(&reference.document);
                // This is the normal managed path; runtime and filesystem permissions
                // enforce readonly until checkout. The historical flag is snapshot-only.
                opened = Some(load(&ws.path(&reference.document).map_err(error)?)?);
                Some(reference.document)
            }
            ManagedTask::OpenArchived { reference } => {
                let catalog = client.managed_catalog().map_err(error)?;
                let entries: BTreeMap<_, _> = catalog.entries.into_iter().map(|entry| (entry.document.clone(), entry)).collect();
                let mut entropy = [0u8; 16];
                getrandom::fill(&mut entropy).map_err(|_| fail("Could not allocate historical snapshot"))?;
                let name: String = entropy.iter().map(|b| format!("{b:02x}")).collect();
                let destination = ws.root().parent().ok_or_else(|| fail("Snapshot parent unavailable"))?.join(format!("history-{name}"));
                let mut history = Workspace::create(&destination, &self.server_id).map_err(error)?;
                let mut queue = vec![VersionRef { document: reference.document.clone(), version: reference.version }];
                let mut resolved = BTreeMap::new();
                while let Some(requested) = queue.pop() {
                    if let Some(previous) = resolved.get(&requested.document) {
                        if *previous != requested.version {
                            return Err(fail("Historical graph needs multiple versions at one path; this snapshot cannot be opened safely"));
                        }
                        continue;
                    }
                    if resolved.len() >= 256 {
                        return Err(fail("Historical dependency limit exceeded"));
                    }
                    let entry = entries.get(&requested.document).ok_or_else(|| fail("Historical document metadata missing"))?;
                    let (metadata, bytes) = client.download(&requested).map_err(error)?;
                    history.install(&requested.document, &entry.logical_path, requested.version, &metadata.sha256, &bytes).map_err(error)?;
                    resolved.insert(requested.document, requested.version);
                    queue.extend(metadata.dependencies);
                }
                let mut session = load(&history.path(&reference.document).map_err(error)?)?;
                session.pdm_read_only = true;
                opened = Some(session);
                let mut state = self.state(&ws, Some(&reference.document))?;
                state.historical_paths = history
                    .manifest()
                    .documents
                    .keys()
                    .map(|document| history.path(document).map(|path| path.to_string_lossy().into_owned()).map_err(error))
                    .collect::<Result<_, _>>()?;
                return Ok((Outcome::Managed { port: self.port, state }, opened));
            }
            ManagedTask::Connect { .. } => return Err(fail("Invalid connection state")),
            ManagedTask::History { document } => Some(document),
            ManagedTask::Archive { path, logical_path, saved, visibility } => {
                let bytes = read_file(Path::new(&path), solidcraft_pdm::MAX_CONTENT).map_err(error)?;
                super::pdm::validate_saved(&bytes, &saved, &visibility)?;
                let absolute = fs::canonicalize(&path).map_err(|_| fail("Saved design unavailable"))?;
                let relative = absolute
                    .strip_prefix(ws.root())
                    .map_err(|_| fail("Save the design inside the configured working folder before archiving"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                if logical_path.as_ref().is_some_and(|p| !p.is_empty() && p != &relative) {
                    return Err(fail("The archive path must match the saved path inside the working folder"));
                }
                let entry = client.managed_create(relative.clone()).map_err(error)?;
                let existing = ws.manifest().documents.get(&entry.document).cloned();
                if let Some(base) = &existing
                    && base.base_sha256 == hash(&bytes)
                    && entry.head == Some(base.base_version)
                    && !base.checked_out
                {
                    return Ok((Outcome::Managed { port: self.port, state: self.state(&ws, Some(&entry.document))? }, None));
                }
                let dependencies = dependencies(&client, &ws, &path, &saved)?;
                let parent = existing.as_ref().map(|e| e.base_version);
                let mut receipt_bytes =
                    serde_json::to_vec(&(ws.manifest().workspace_id.clone(), entry.document.clone(), parent, hash(&bytes), &dependencies))
                        .map_err(|_| fail("Could not identify publication"))?;
                let request_id = hash(&receipt_bytes);
                receipt_bytes.clear();
                // Reconcile an acknowledged-on-server publication before obtaining a new lock.
                let published = client
                    .managed_history(entry.document.clone())
                    .map_err(error)?
                    .into_iter()
                    .find(|v| v.request_id == request_id && v.version.sha256 == hash(&bytes));
                let published = match published {
                    Some(version) => version,
                    None => {
                        let grant = if let Some(grant) = self.grants.get(&entry.document) {
                            grant.clone()
                        } else {
                            if existing.is_some() {
                                return Err(fail("Check out this managed design before archiving changes"));
                            }
                            let grant = client
                                .managed_checkout(entry.document.clone(), ws.manifest().workspace_id.clone(), self.machine.clone())
                                .map_err(error)?;
                            self.grants.insert(entry.document.clone(), grant.clone());
                            grant
                        };
                        client
                            .managed_check_in(ManagedCheckIn {
                                document: entry.document.clone(),
                                workspace: ws.manifest().workspace_id.clone(),
                                machine: self.machine.clone(),
                                checkout_token: grant.token,
                                request_id,
                                parent,
                                content: bytes,
                                dependencies,
                                comment: String::new(),
                            })
                            .map_err(error)?
                    }
                };
                if existing.is_some() {
                    ws.record_checkin(&entry.document, published.version.reference.version, &published.version.sha256).map_err(error)?;
                } else {
                    ws.adopt(&entry.document, &relative, published.version.reference.version, &published.version.sha256).map_err(error)?;
                }
                self.grants.remove(&entry.document);
                Some(entry.document)
            }
            ManagedTask::Checkout { document, path } => {
                let document = self.target(&ws, document, path)?;
                let grant = client.managed_checkout(document.clone(), ws.manifest().workspace_id.clone(), self.machine.clone()).map_err(error)?;
                self.grants.insert(document.clone(), grant.clone());
                if !ws.manifest().documents.get(&document).is_some_and(|e| e.checked_out || Some(e.base_version) == grant.entry.head) {
                    self.install_graph(&client, &mut ws, &document)?;
                }
                ws.checkout(&document).map_err(error)?;
                opened = Some(load(&ws.path(&document).map_err(error)?)?);
                Some(document)
            }
            ManagedTask::GetLatest { document, path } => {
                let document = self.target(&ws, document, path)?;
                self.install_graph(&client, &mut ws, &document)?;
                let mut loaded = load(&ws.path(&document).map_err(error)?)?;
                if !loaded.doc.links.is_empty() {
                    loaded
                        .execute("component.update_links", &serde_json::json!({}))
                        .map_err(|_| fail("Files retrieved; assembly update failed. Review links before saving"))?;
                }
                opened = Some(loaded);
                Some(document)
            }
            ManagedTask::Open { document } => {
                if !ws.manifest().documents.contains_key(&document) {
                    self.install_graph(&client, &mut ws, &document)?;
                }
                let mut loaded = load(&ws.path(&document).map_err(error)?)?;
                if !loaded.doc.links.is_empty() {
                    loaded
                        .execute("component.update_links", &serde_json::json!({}))
                        .map_err(|_| fail("The working assembly could not update its local components"))?;
                }
                opened = Some(loaded);
                Some(document)
            }
            ManagedTask::CancelCheckout { document, path } => {
                let document = self.target(&ws, document, path)?;
                let grant = self.grants.get(&document).cloned().ok_or_else(|| fail("Reconnect and confirm your checkout before undoing it"))?;
                if !ws.manifest().documents.contains_key(&document) {
                    client
                        .managed_cancel_checkout(document.clone(), ws.manifest().workspace_id.clone(), self.machine.clone(), grant.token)
                        .map_err(error)?;
                    self.grants.remove(&document);
                    return Ok((Outcome::Managed { port: self.port, state: self.state(&ws, Some(&document))? }, None));
                }
                let entry = ws.manifest().documents.get(&document).cloned().ok_or_else(|| fail("Working copy is missing"))?;
                let (_, bytes) = client.download(&VersionRef { document: document.clone(), version: entry.base_version }).map_err(error)?;
                ws.undo_checkout(&document, &bytes, true).map_err(error)?;
                let reserved = client
                    .managed_catalog()
                    .map_err(error)?
                    .entries
                    .into_iter()
                    .find(|e| e.document == document)
                    .is_some_and(|e| e.checkout.as_ref().is_some_and(|checkout| Some(checkout) == grant.entry.checkout.as_ref()));
                if reserved {
                    client
                        .managed_cancel_checkout(document.clone(), ws.manifest().workspace_id.clone(), self.machine.clone(), grant.token)
                        .map_err(error)?;
                }
                self.grants.remove(&document);
                opened = Some(load(&ws.path(&document).map_err(error)?)?);
                Some(document)
            }
        };
        Ok((Outcome::Managed { port: self.port, state: self.state(&ws, selected.as_deref())? }, opened))
    }

    fn install_graph(&self, client: &Client, ws: &mut Workspace, root: &str) -> Result<(), Failure> {
        let catalog = client.managed_catalog().map_err(error)?;
        let entries: BTreeMap<_, _> = catalog.entries.into_iter().map(|e| (e.document.clone(), e)).collect();
        let mut queue = vec![root.to_string()];
        let mut seen = BTreeSet::new();
        let mut downloads = Vec::new();
        while let Some(document) = queue.pop() {
            if !seen.insert(document.clone()) {
                continue;
            }
            if seen.len() > 256 {
                return Err(fail("Assembly exceeds managed dependency limit"));
            }
            let entry = entries.get(&document).ok_or_else(|| fail("A managed dependency is missing"))?;
            let version = entry.head.ok_or_else(|| fail("The selected document has no archived version"))?;
            if let Some(old) = ws.manifest().documents.get(&document)
                && (old.checked_out || ws.local_sha256(&document).map_err(error)? != old.base_sha256)
            {
                return Err(fail("Update refused: a local dependency is modified or checked out"));
            }
            let (metadata, bytes) = client.download(&VersionRef { document: document.clone(), version }).map_err(error)?;
            if hash(&bytes) != metadata.sha256 {
                return Err(fail("Dependency integrity verification failed"));
            }
            queue.extend(metadata.dependencies.iter().map(|r| r.document.clone()));
            downloads.push((entry.clone(), metadata, bytes));
        }
        // Every participating file is checked before any replacement. Each individual
        // installation is journaled; a later I/O failure leaves accurately recorded bases.
        for (entry, metadata, bytes) in downloads {
            ws.install(&entry.document, &entry.logical_path, metadata.reference.version, &metadata.sha256, &bytes).map_err(error)?;
        }
        Ok(())
    }
}

fn restore_receipt(workspace: &str, reference: &Reference, parent: u64, sha256: &str, dependencies: &[VersionRef]) -> Result<String, Failure> {
    let bytes = serde_json::to_vec(&("managed-restore/1", workspace, &reference.document, reference.version, parent, sha256, dependencies))
        .map_err(|_| fail("Could not identify historical restoration"))?;
    Ok(hash(&bytes))
}
fn load(path: &Path) -> Result<Box<Session>, Failure> {
    let mut session = Session::default();
    session.execute("doc.open", &serde_json::json!({"path":path})).map_err(|_| fail("The managed copy is not a readable native design"))?;
    Ok(Box::new(session))
}

fn dependencies(
    client: &Client,
    ws: &Workspace,
    path: &str,
    saved: &std::sync::Arc<solidcraft_engine::doc::Document>,
) -> Result<Vec<VersionRef>, Failure> {
    let mut references = BTreeSet::new();
    for link in saved.links.values() {
        let source = Path::new(path).parent().ok_or_else(|| fail("Saved design has no folder"))?.join(&link.path);
        let source = fs::canonicalize(source).map_err(|_| fail("A linked native component is missing"))?;
        let document = ws
            .manifest()
            .documents
            .keys()
            .find(|document| ws.path(document).ok().and_then(|p| fs::canonicalize(p).ok()).as_ref() == Some(&source))
            .ok_or_else(|| fail("Archive each linked component in this workspace before archiving the assembly"))?;
        let entry = ws.manifest().documents.get(document).ok_or_else(|| fail("Dependency metadata is missing"))?;
        if ws.local_sha256(document).map_err(error)? != entry.base_sha256 {
            return Err(fail("Archive local component edits before archiving the assembly"));
        }
        let reference = VersionRef { document: document.clone(), version: entry.base_version };
        let (version, bytes) = client.download(&reference).map_err(error)?;
        if version.sha256 != entry.base_sha256 || hash(&bytes) != entry.base_sha256 {
            return Err(fail("Dependency base does not match the archived content"));
        }
        references.insert(reference);
    }
    if !saved.links.is_empty() {
        let mut probe = Session::default();
        probe.doc = saved.clone();
        probe.path = Some(path.into());
        probe.execute("component.update_links", &serde_json::json!({})).map_err(|_| fail("Could not validate the assembly's consumed components"))?;
        if *probe.doc != **saved {
            return Err(fail("Update native links and save the assembly before archiving; its embedded components are out of date"));
        }
    }
    Ok(references.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Trial {
        root: PathBuf,
        port: u16,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl Trial {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("solidcraft-managed-native-{}-{}", std::process::id(), AuthToken::generate().unwrap()));
            fs::create_dir(&root).unwrap();
            let mut store = solidcraft_pdm::Store::open(&root.join("store")).unwrap();
            let listener = solidcraft_pdm::bind_loopback(0).unwrap();
            let port = listener.local_addr().unwrap().port();
            let stop = Arc::new(AtomicBool::new(false));
            let stopped = stop.clone();
            let thread = std::thread::spawn(move || {
                for connection in listener.incoming() {
                    if stopped.load(Ordering::Acquire) {
                        break;
                    }
                    if let Ok(stream) = connection {
                        let _ = solidcraft_pdm::serve_connection(&mut store, stream, &token());
                    }
                }
            });
            Self { root, port, stop, thread: Some(thread) }
        }
        fn host(&self, name: &str) -> Host {
            let client = Client::new(self.port, token());
            let catalog = client.managed_catalog().unwrap();
            let root = self.root.join(name);
            let ws = Workspace::create(&root, &catalog.server_id).unwrap();
            drop(ws);
            Host {
                client: Some(client),
                port: self.port,
                root: Some(root),
                server_id: catalog.server_id,
                machine: "test-desktop".into(),
                grants: BTreeMap::new(),
            }
        }
    }
    impl Drop for Trial {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            let _ = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, self.port));
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
            // Restore permissions only inside the unique disposable test directory.
            fn writable(path: &Path) {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            writable(&p);
                        } else if let Ok(meta) = fs::metadata(&p) {
                            let mut permissions = meta.permissions();
                            #[allow(clippy::permissions_set_readonly_false)]
                            permissions.set_readonly(false);
                            let _ = fs::set_permissions(&p, permissions);
                        }
                    }
                }
            }
            writable(&self.root);
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn token() -> AuthToken {
        AuthToken::parse("ab".repeat(32)).unwrap()
    }
    fn run(host: &mut Host, task: ManagedTask) -> (Outcome, Option<Box<Session>>) {
        host.execute(task, None).unwrap_or_else(|e| panic!("{}", e.message))
    }
    fn archive(host: &mut Host, session: &Session) -> String {
        let (outcome, _) = run(
            host,
            ManagedTask::Archive {
                path: session.path.clone().unwrap(),
                logical_path: None,
                saved: session.doc.clone(),
                visibility: Box::new(session.visibility.clone()),
            },
        );
        match outcome {
            Outcome::Managed { state, .. } => state
                .local
                .iter()
                .find(|e| session.path.as_deref().is_some_and(|path| solidcraft_engine::pdm::same_file_path(&e.path, path)))
                .unwrap()
                .document
                .clone(),
            _ => panic!("unexpected outcome"),
        }
    }
    fn part(path: &Path) -> Session {
        let mut session = Session::default();
        session.execute("solid.box", &serde_json::json!({"length":10,"width":20,"height":3})).unwrap();
        session.execute("file.save", &serde_json::json!({"path":path})).unwrap();
        session
    }
    #[test]
    fn managed_native_keeps_one_path_excludes_other_workspace_and_opens_history_separately() {
        let trial = Trial::new();
        let mut host = trial.host("working");
        let mut other = trial.host("other");
        let path = host.root.as_ref().unwrap().join("part.solidcraft");
        let session = part(&path);
        let original = fs::read(&path).unwrap();
        let document = archive(&mut host, &session);
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        let (_, loaded) = run(&mut host, ManagedTask::Checkout { document: Some(document.clone()), path: None });
        let mut session = *loaded.unwrap();
        assert!(solidcraft_engine::pdm::same_file_path(session.path.as_ref().unwrap(), path.to_str().unwrap()));
        assert!(other.execute(ManagedTask::Checkout { document: Some(document.clone()), path: None }, None).is_err());
        session.execute("solid.box", &serde_json::json!({"length":2,"width":2,"height":2})).unwrap();
        session.execute("file.save", &serde_json::json!({})).unwrap();
        archive(&mut host, &session);
        assert_eq!(host.client().unwrap().managed_history(document.clone()).unwrap().len(), 2);
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        let (outcome, loaded) = run(&mut host, ManagedTask::OpenArchived { reference: Reference { document, version: 1 } });
        let historic = loaded.unwrap();
        assert_ne!(historic.path.as_ref(), session.path.as_ref());
        assert!(historic.pdm_read_only);
        let Outcome::Managed { state, .. } = outcome else { panic!("expected managed snapshot") };
        assert_eq!(state.historical_paths, vec![historic.path.clone().unwrap()]);
        assert_eq!(fs::read(historic.path.unwrap()).unwrap(), original);
        assert_ne!(fs::read(&path).unwrap(), original);
    }
    #[test]
    fn undo_restores_same_path_and_dirty_get_latest_preserves_disk_bytes() {
        let trial = Trial::new();
        let mut host = trial.host("working");
        let path = host.root.as_ref().unwrap().join("part.solidcraft");
        let session = part(&path);
        let original = fs::read(&path).unwrap();
        let document = archive(&mut host, &session);
        let (_, loaded) = run(&mut host, ManagedTask::Checkout { document: Some(document.clone()), path: None });
        let mut session = *loaded.unwrap();
        session.execute("solid.box", &serde_json::json!({"length":2,"width":2,"height":2})).unwrap();
        session.execute("file.save", &serde_json::json!({})).unwrap();
        let edited = fs::read(&path).unwrap();
        assert!(host.execute(ManagedTask::GetLatest { document: Some(document.clone()), path: None }, None).is_err());
        assert_eq!(fs::read(&path).unwrap(), edited);
        run(&mut host, ManagedTask::CancelCheckout { document: Some(document.clone()), path: None });
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(host.client().unwrap().managed_catalog().unwrap().entries.into_iter().find(|e| e.document == document).unwrap().checkout.is_none());
        assert_eq!(host.client().unwrap().managed_history(document).unwrap().len(), 1);
    }

    #[test]
    fn working_assembly_updates_components_while_historical_manifest_retains_consumed_versions() {
        let trial = Trial::new();
        let mut host = trial.host("working");
        let mut other = trial.host("other");
        let root = host.root.clone().unwrap();
        let part_path = root.join("part.solidcraft");
        let original_part = part(&part_path);
        let part_id = archive(&mut host, &original_part);
        let assembly_path = root.join("assembly.solidcraft");
        let mut assembly = Session::default();
        assembly.execute("file.save", &serde_json::json!({"path":assembly_path})).unwrap();
        assembly.execute("component.insert_design", &serde_json::json!({"path":part_path})).unwrap();
        assembly.execute("file.save", &serde_json::json!({})).unwrap();
        let original_doc = assembly.doc.clone();
        let assembly_id = archive(&mut host, &assembly);
        let archived = host.client().unwrap().managed_history(assembly_id.clone()).unwrap();
        assert_eq!(archived[0].version.dependencies, vec![VersionRef { document: part_id.clone(), version: 1 }]);
        let (_, part_copy) = run(&mut host, ManagedTask::Checkout { document: Some(part_id.clone()), path: None });
        let mut changed = *part_copy.unwrap();
        changed.execute("solid.box", &serde_json::json!({"length":2,"width":2,"height":2})).unwrap();
        changed.execute("file.save", &serde_json::json!({})).unwrap();
        // Local saved component changes propagate without a check-in.
        assembly.execute("component.update_links", &serde_json::json!({})).unwrap();
        assert_ne!(*assembly.doc, *original_doc);
        archive(&mut host, &changed);
        let (_, working) = run(&mut other, ManagedTask::Open { document: assembly_id.clone() });
        let working = working.unwrap();
        assert_ne!(*working.doc, *original_doc);
        assert!(working.is_dirty());
        let (_, historic) = run(&mut host, ManagedTask::OpenArchived { reference: Reference { document: assembly_id, version: 1 } });
        let historic = historic.unwrap();
        assert_eq!(*historic.doc, *original_doc);
        assert!(historic.pdm_read_only);
        let history_root = Path::new(historic.path.as_ref().unwrap()).parent().unwrap();
        assert_eq!(
            fs::read(history_root.join("part.solidcraft")).unwrap(),
            host.client().unwrap().download(&VersionRef { document: part_id, version: 1 }).unwrap().1
        );
    }
    #[test]
    fn restore_history_publishes_new_head_once_and_preserves_normal_path() {
        let trial = Trial::new();
        let mut host = trial.host("working");
        let path = host.root.as_ref().unwrap().join("part.solidcraft");
        let original = part(&path);
        let original_bytes = fs::read(&path).unwrap();
        let document = archive(&mut host, &original);
        let (_, loaded) = run(&mut host, ManagedTask::Checkout { document: Some(document.clone()), path: None });
        let mut changed = *loaded.unwrap();
        changed.execute("solid.box", &serde_json::json!({"length":2,"width":2,"height":2})).unwrap();
        changed.execute("file.save", &serde_json::json!({})).unwrap();
        let changed_bytes = fs::read(&path).unwrap();
        archive(&mut host, &changed);
        let task = ManagedTask::Restore { reference: Reference { document: document.clone(), version: 1 } };
        let (_, loaded) = run(&mut host, task.clone());
        let restored = loaded.unwrap();
        assert!(solidcraft_engine::pdm::same_file_path(restored.path.as_ref().unwrap(), path.to_str().unwrap()));
        assert!(!restored.pdm_read_only);
        assert_eq!(fs::read(&path).unwrap(), original_bytes);
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        let history = host.client().unwrap().managed_history(document.clone()).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[2].version.parent, Some(2));
        assert_eq!(history[2].version.reference.version, 3);
        assert_eq!(host.client().unwrap().download(&VersionRef { document: document.clone(), version: 2 }).unwrap().1, changed_bytes);
        run(&mut host, task.clone());
        assert_eq!(host.client().unwrap().managed_history(document.clone()).unwrap().len(), 3);
        let (_, editable) = run(&mut host, ManagedTask::Checkout { document: Some(document.clone()), path: None });
        let editable = editable.unwrap();
        assert!(!editable.pdm_read_only);
        assert!(solidcraft_engine::pdm::same_file_path(editable.path.as_ref().unwrap(), path.to_str().unwrap()));
        assert!(!fs::metadata(&path).unwrap().permissions().readonly());
        assert!(
            host.client()
                .unwrap()
                .managed_catalog()
                .unwrap()
                .entries
                .into_iter()
                .find(|entry| entry.document == document)
                .unwrap()
                .checkout
                .is_some()
        );
        run(&mut host, ManagedTask::CancelCheckout { document: Some(document.clone()), path: None });
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        assert_eq!(fs::read(&path).unwrap(), original_bytes);
        assert_eq!(host.client().unwrap().managed_history(document.clone()).unwrap().len(), 3);
        assert!(
            host.client()
                .unwrap()
                .managed_catalog()
                .unwrap()
                .entries
                .into_iter()
                .find(|entry| entry.document == document)
                .unwrap()
                .checkout
                .is_none()
        );
        // Simulate an external edit bypassing the readonly convenience bit.
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(&path, permissions).unwrap();
        fs::write(&path, b"external unsaved-to-archive edits").unwrap();
        assert!(host.execute(task, None).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"external unsaved-to-archive edits");
        assert_eq!(host.client().unwrap().managed_history(document).unwrap().len(), 3);
    }

    #[test]
    fn restore_reconciles_lost_publish_reply_before_acquiring_another_grant() {
        let trial = Trial::new();
        let mut host = trial.host("working");
        let path = host.root.as_ref().unwrap().join("part.solidcraft");
        let original = part(&path);
        let document = archive(&mut host, &original);
        let (_, loaded) = run(&mut host, ManagedTask::Checkout { document: Some(document.clone()), path: None });
        let mut changed = *loaded.unwrap();
        changed.execute("solid.box", &serde_json::json!({"length":2,"width":2,"height":2})).unwrap();
        changed.execute("file.save", &serde_json::json!({})).unwrap();
        archive(&mut host, &changed);
        let client = host.client().unwrap();
        let reference = Reference { document: document.clone(), version: 1 };
        let (source, bytes) = client.download(&VersionRef { document: document.clone(), version: 1 }).unwrap();
        let ws = host.workspace().unwrap();
        let workspace = ws.manifest().workspace_id.clone();
        let receipt = restore_receipt(&workspace, &reference, 2, &source.sha256, &source.dependencies).unwrap();
        drop(ws);
        let grant = client.managed_checkout(document.clone(), workspace.clone(), host.machine.clone()).unwrap();
        // The server commits, but the caller loses its response and does not change local state.
        client
            .managed_check_in(ManagedCheckIn {
                document: document.clone(),
                workspace,
                machine: host.machine.clone(),
                checkout_token: grant.token,
                request_id: receipt,
                parent: Some(2),
                content: bytes.clone(),
                dependencies: source.dependencies,
                comment: "Restore archived version 1".into(),
            })
            .unwrap();
        assert_eq!(host.workspace().unwrap().manifest().documents.get(&document).unwrap().base_version, 2);
        // A different workspace may acquire a new grant after publication. Reconciliation
        // must not attempt to acquire it or release this other workspace's reservation.
        let other = trial.host("other");
        let other_ws = other.workspace().unwrap();
        let other_grant = client.managed_checkout(document.clone(), other_ws.manifest().workspace_id.clone(), other.machine.clone()).unwrap();
        drop(other_ws);
        run(&mut host, ManagedTask::Restore { reference });
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        assert_eq!(client.managed_history(document.clone()).unwrap().len(), 3);
        assert_eq!(
            client.managed_catalog().unwrap().entries.into_iter().find(|e| e.document == document).unwrap().checkout,
            other_grant.entry.checkout
        );
    }
}
