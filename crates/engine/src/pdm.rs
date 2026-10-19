//! Host-only local PDM work queue. No credentials, sockets or document serialization live here.
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub document: String,
    pub version: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Version {
    pub reference: Reference,
    pub parent: Option<u64>,
    pub sha256: String,
    pub dependencies: Vec<Reference>,
}

/// Public server metadata only. Reservation credentials never enter engine state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkout {
    pub owner: String,
    pub workspace: String,
    pub machine: String,
    pub checked_out_unix_ms: u64,
    pub base_version: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub document: String,
    pub logical_path: String,
    pub head: Option<u64>,
    pub checkout: Option<Checkout>,
    #[serde(default)]
    pub head_checked_in_unix_ms: Option<u64>,
    #[serde(default)]
    pub head_author: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalEntry {
    pub document: String,
    pub path: String,
    pub relative_path: String,
    pub base_version: u64,
    pub sha256: String,
    pub checked_out: bool,
    pub modified: bool,
    pub present: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagedVersion {
    pub version: Version,
    pub author: String,
    pub checked_in_unix_ms: u64,
    pub comment: String,
    pub request_id: String,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct ManagedState {
    pub server_id: Option<String>,
    pub root: Option<String>,
    pub workspace: Option<String>,
    pub catalog: Vec<CatalogEntry>,
    pub local: Vec<LocalEntry>,
    pub history: Vec<ManagedVersion>,
    pub where_used: Vec<Reference>,
    pub preview: Option<Preview>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preview {
    pub reference: Reference,
    pub sha256: String,
    pub png: Vec<u8>,
}

#[derive(Clone)]
pub enum ManagedTask {
    Recover { port: u16, root: String },
    Preview { reference: Reference },
    OpenArchived { reference: Reference },
    Restore { reference: Reference },
    Connect { port: u16, root: Option<String> },
    Archive { path: String, logical_path: Option<String>, saved: Arc<solidcraft_doc::Document>, visibility: Box<crate::Visibility> },
    Checkout { document: Option<String>, path: Option<String> },
    CancelCheckout { document: Option<String>, path: Option<String> },
    GetLatest { document: Option<String>, path: Option<String> },
    Open { document: String },
    History { document: String },
}

#[derive(Clone)]
pub enum Task {
    Managed(ManagedTask),
    Connect {
        port: u16,
    },
    Disconnect,
    Refresh,
    CheckIn {
        document: String,
        parent: Option<u64>,
        path: String,
        dependencies: Vec<Reference>,
        saved: Arc<solidcraft_doc::Document>,
        visibility: Box<crate::Visibility>,
    },
    Retrieve {
        reference: Reference,
        open: bool,
    },
}
impl std::fmt::Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Managed(_) => "Managed([host-only task])",
            Self::Connect { .. } => "Connect",
            Self::Disconnect => "Disconnect",
            Self::Refresh => "Refresh",
            Self::CheckIn { .. } => "CheckIn([host-only snapshot])",
            Self::Retrieve { .. } => "Retrieve",
        })
    }
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: u64,
    pub task: Task,
}

#[derive(Clone, Debug, Serialize)]
pub struct Retrieved {
    pub reference: Reference,
    pub path: String,
    pub versions: Vec<Version>,
}

#[derive(Clone, Debug)]
pub enum Outcome {
    ManagedRefreshed { port: u16, state: ManagedState },
    Managed { port: u16, state: ManagedState },
    Connected { port: u16, versions: Vec<Version> },
    Disconnected,
    Listed(Vec<Version>),
    CheckedIn(Version),
    Retrieved(Retrieved),
}

#[derive(Clone, Debug)]
pub struct Failure {
    pub message: String,
    pub disconnect: bool,
}

#[derive(Clone, Default, Debug, Serialize)]
pub struct Status {
    pub available: bool,
    pub port: Option<u16>,
    pub busy: Option<u64>,
    /// Only local file replacement/publication prevents editing. Metadata refreshes do not.
    pub blocking: bool,
    pub versions: Vec<Version>,
    pub retrieved: Option<Retrieved>,
    pub error: Option<String>,
    #[serde(flatten)]
    pub managed: ManagedState,
}

#[derive(Default)]
struct State {
    status: Status,
    next: u64,
    job: Option<Job>,
    blocked_paths: Vec<String>,
}

#[derive(Clone, Default)]
pub struct Runtime(Arc<Mutex<State>>);

pub fn valid_document(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}

/// Cached display-path identity, without filesystem access on UI frames.
pub fn cached_path_key(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    let normalized = if cfg!(target_os = "windows") { normalized.strip_prefix("//?/").unwrap_or(&normalized) } else { &normalized };
    let rooted = normalized.starts_with('/');
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p: &&str| *p != ".." && !p.ends_with(':')) => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let mut result = parts.join("/");
    if rooted {
        result.insert(0, '/');
    }
    if cfg!(target_os = "windows") {
        result.make_ascii_lowercase();
    }
    result
}

/// Save-time identity check. The UI uses cached paths; filesystem resolution is
/// limited to operations that already perform filesystem writes.
pub fn same_file_path(a: &str, b: &str) -> bool {
    fn resolve(value: &str) -> std::path::PathBuf {
        let path = std::path::Path::new(value);
        std::fs::canonicalize(path).unwrap_or_else(|_| {
            path.parent()
                .and_then(|p| std::fs::canonicalize(p).ok())
                .and_then(|p| path.file_name().map(|n| p.join(n)))
                .unwrap_or_else(|| path.to_path_buf())
        })
    }
    let a = resolve(a).to_string_lossy().replace('\\', "/");
    let b = resolve(b).to_string_lossy().replace('\\', "/");
    if cfg!(target_os = "windows") { a.eq_ignore_ascii_case(&b) } else { a == b }
}

impl Reference {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_document(&self.document) || self.version == 0 {
            return Err("Invalid PDM version reference".into());
        }
        Ok(())
    }
}

impl Runtime {
    pub fn set_available(&self, available: bool) -> Result<(), String> {
        self.0.lock().map_err(|_| "Local PDM state unavailable")?.status.available = available;
        Ok(())
    }
    pub fn status(&self) -> Result<Status, String> {
        Ok(self.0.lock().map_err(|_| "Local PDM state unavailable")?.status.clone())
    }
    pub fn enqueue(&self, task: Task) -> Result<u64, String> {
        let mut state = self.0.lock().map_err(|_| "Local PDM state unavailable")?;
        if !state.status.available {
            return Err("Local PDM is unavailable in this host".into());
        }
        if state.status.busy.is_some() {
            return Err("A local PDM operation is in progress".into());
        }
        if !matches!(task, Task::Connect { .. } | Task::Managed(ManagedTask::Connect { .. } | ManagedTask::Recover { .. }))
            && state.status.port.is_none()
        {
            return Err("Connect to a local PDM service first".into());
        }
        state.next = state.next.checked_add(1).ok_or("Local PDM request limit reached")?;
        let id = state.next;
        state.blocked_paths = state.status.managed.local.iter().map(|entry| entry.path.clone()).collect();
        if let Task::Managed(ManagedTask::Archive { path, .. }) | Task::CheckIn { path, .. } = &task {
            state.blocked_paths.push(path.clone());
        }
        state.job = Some(Job { id, task });
        state.status.blocking = state.job.as_ref().is_some_and(|job| {
            matches!(
                &job.task,
                Task::CheckIn { .. }
                    | Task::Managed(
                        ManagedTask::Archive { .. }
                            | ManagedTask::Checkout { .. }
                            | ManagedTask::CancelCheckout { .. }
                            | ManagedTask::GetLatest { .. }
                            | ManagedTask::Open { .. }
                            | ManagedTask::Recover { .. }
                    )
            )
        });
        state.status.busy = Some(id);
        state.status.error = None;
        Ok(id)
    }
    pub fn take_job(&self) -> Result<Option<Job>, String> {
        Ok(self.0.lock().map_err(|_| "Local PDM state unavailable")?.job.take())
    }
    pub fn finish(&self, id: u64, result: Result<Outcome, Failure>) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "Local PDM state unavailable")?;
        if state.status.busy != Some(id) {
            return Err("Stale local PDM result".into());
        }
        state.status.busy = None;
        state.status.blocking = false;
        state.blocked_paths.clear();
        state.job = None;
        match result {
            Err(failure) => {
                state.status.error = Some(failure.message);
                if failure.disconnect {
                    state.status.port = None;
                    state.status.versions.clear();
                }
            }
            Ok(Outcome::Disconnected) => {
                state.status.port = None;
                state.status.versions.clear();
                state.status.retrieved = None;
            }
            Ok(Outcome::Managed { port, state: managed }) => {
                state.status.port = Some(port);
                state.status.managed = managed;
            }
            Ok(Outcome::ManagedRefreshed { port, state: mut managed }) => {
                if state.status.managed.server_id == managed.server_id {
                    managed.history = state.status.managed.history.clone();
                    managed.where_used = state.status.managed.where_used.clone();
                    managed.preview = state.status.managed.preview.clone();
                }
                state.status.port = Some(port);
                state.status.managed = managed;
            }
            Ok(Outcome::Connected { port, versions }) => {
                state.status.port = Some(port);
                state.status.versions = versions;
            }
            Ok(Outcome::Listed(versions)) => state.status.versions = versions,
            Ok(Outcome::CheckedIn(version)) => {
                state.status.versions.retain(|v| v.reference != version.reference);
                state.status.versions.push(version);
            }
            Ok(Outcome::Retrieved(retrieved)) => state.status.retrieved = Some(retrieved),
        }
        state.status.versions.sort_by(|a, b| (&a.reference.document, a.reference.version).cmp(&(&b.reference.document, b.reference.version)));
        Ok(())
    }

    pub fn managed_for_path(&self, path: &str) -> Option<LocalEntry> {
        let key = cached_path_key(path);
        self.status().ok()?.managed.local.into_iter().find(|entry| cached_path_key(&entry.path) == key)
    }

    pub fn blocks_path(&self, path: &str) -> bool {
        self.0.lock().map_or(true, |state| {
            state.status.blocking && state.blocked_paths.iter().any(|blocked| cached_path_key(blocked) == cached_path_key(path))
        })
    }

    pub fn may_save(&self, path: &str) -> Result<(), String> {
        let status = self.status()?;
        let blocked = self.0.lock().map_err(|_| "Local PDM state unavailable")?.blocked_paths.clone();
        if status.blocking && blocked.iter().any(|blocked| same_file_path(blocked, path)) {
            return Err("Wait for the managed operation before saving this file".into());
        }
        if status.managed.local.iter().any(|entry| !entry.checked_out && same_file_path(&entry.path, path)) {
            return Err("Extract this managed design before saving, or Save As a separate copy".into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "pdm_tests.rs"]
mod tests;
