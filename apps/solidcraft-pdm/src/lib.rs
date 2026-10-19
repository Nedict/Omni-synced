//! Experimental, single-writer PDM storage and bounded JSON-lines loopback protocol.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub mod backup_restore;
pub mod blob_store;
pub mod bootstrap;
pub mod client;
pub mod http_server;
pub mod managed;
pub mod managed_workspace;
pub mod workspace;

pub const PROTOCOL: &str = "solidcraft-pdm/2";
pub const HTTP_RPC_PATH: &str = "/pdm/v2/rpc";

/// HTTPS carries authentication in a bearer header, separate from this payload.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RpcEnvelope {
    pub protocol: String,
    pub request: Request,
}
pub const TOKEN_ENV: &str = "SOLIDCRAFT_PDM_TOKEN";

/// A local owner credential. Never include its contents in Debug or error messages.
#[derive(Clone)]
pub struct AuthToken(String);

impl std::fmt::Debug for AuthToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthToken([redacted])")
    }
}

impl AuthToken {
    pub fn parse(value: String) -> Result<Self, Error> {
        if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Invalid("owner token must contain 64 hexadecimal characters"));
        }
        Ok(Self(value))
    }

    pub fn from_env() -> Result<Self, Error> {
        Self::parse(std::env::var(TOKEN_ENV).map_err(|_| Error::Invalid("SOLIDCRAFT_PDM_TOKEN is required"))?)
    }

    /// Explicit bootstrap command only: callers must capture stdout privately, never log it.
    pub fn generate() -> Result<String, Error> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| Error::Invalid("OS randomness unavailable"))?;
        Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }

    fn matches(&self, provided: &str) -> bool {
        use subtle::ConstantTimeEq;
        provided.len() == 64 && bool::from(self.0.as_bytes().ct_eq(provided.as_bytes()))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedRequest {
    protocol: String,
    token: String,
    request: Request,
}

impl AuthenticatedRequest {
    pub fn new(token: &AuthToken, request: Request) -> Self {
        Self { protocol: PROTOCOL.into(), token: token.0.clone(), request }
    }
}

pub const MAX_CONTENT: usize = 1024 * 1024;
pub const MAX_FRAME: u64 = 8 * 1024 * 1024;
const MAX_DEPENDENCIES: usize = 256;
pub const MAX_VERSIONS: usize = 256;
pub const MAX_STORED_CONTENT: u64 = 64 * 1024 * 1024;

/// The transport intentionally has no bind-address option while it lacks authentication.
pub fn bind_loopback(port: u16) -> Result<TcpListener, Error> {
    Ok(TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))?)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("local PDM connection failed: {0}")]
    Network(#[source] std::io::Error),
    #[error("server rejected the request: {0}")]
    Server(String),
    #[error("invalid request: {0}")]
    Invalid(&'static str),
    #[error("operation forbidden")]
    Forbidden,
    #[error("version conflict")]
    Conflict,
    #[error("version not found")]
    NotFound,
    #[error("storage integrity failure")]
    Integrity,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionRef {
    pub document: String,
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    pub reference: VersionRef,
    pub parent: Option<u64>,
    pub sha256: String,
    pub dependencies: Vec<VersionRef>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    ManagedCatalog,
    ManagedCreate { logical_path: String },
    ManagedCheckout { document: String, workspace: String, machine: String },
    ManagedCancelCheckout { document: String, workspace: String, machine: String, checkout_token: String },
    ManagedCheckIn { input: managed::ManagedCheckIn },
    ManagedHistory { document: String },
    CheckIn { document: String, parent: Option<u64>, content: Vec<u8>, dependencies: Vec<VersionRef> },
    Download { reference: VersionRef },
    WhereUsed { reference: VersionRef },
    List { document: Option<String> },
}

impl std::fmt::Debug for Request {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let operation = match self {
            Self::ManagedCatalog => "ManagedCatalog",
            Self::ManagedCreate { .. } => "ManagedCreate",
            Self::ManagedCheckout { .. } => "ManagedCheckout",
            Self::ManagedCancelCheckout { .. } => "ManagedCancelCheckout",
            Self::ManagedCheckIn { .. } => "ManagedCheckIn",
            Self::ManagedHistory { .. } => "ManagedHistory",
            Self::CheckIn { .. } => "CheckIn",
            Self::Download { .. } => "Download",
            Self::WhereUsed { .. } => "WhereUsed",
            Self::List { .. } => "List",
        };
        formatter.write_str(operation)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Response {
    ManagedCatalog { catalog: managed::ManagedCatalog },
    ManagedDocument { entry: managed::CatalogEntry },
    ManagedCheckout { grant: managed::CheckoutGrant },
    ManagedCheckedIn { version: managed::ManagedVersion },
    ManagedHistory { versions: Vec<managed::ManagedVersion> },
    Listed { versions: Vec<Version> },
    CheckedIn { version: Version },
    Downloaded { version: Version, content: Vec<u8> },
    WhereUsed { versions: Vec<VersionRef> },
    Error { code: String },
}

/// A process-wide OS lock prevents two servers from allocating the same next version.
/// Each immutable version directory is published with one rename on the same filesystem.
pub struct Store {
    root: PathBuf,
    _lock: File,
    managed: managed::ManagedStore,
}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

pub(crate) fn digest(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

impl Store {
    pub fn list(&self, document: Option<&str>) -> Result<Vec<Version>, Error> {
        let mut result = Vec::new();
        if let Some(document) = document {
            for version in self.versions(document)? {
                result.push(self.download(&VersionRef { document: document.into(), version })?.0);
            }
        } else {
            for entry in fs::read_dir(&self.root)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Some(document) = name.strip_prefix("d-").filter(|id| valid_id(id)) {
                    result.extend(self.list(Some(document))?);
                }
            }
        }
        result.extend(self.managed.versions().into_iter().filter(|v| document.is_none_or(|d| v.reference.document == d)));
        result.sort_by(|a, b| a.reference.cmp(&b.reference));
        result.dedup_by(|a, b| a.reference == b.reference);
        Ok(result)
    }

    fn check_quota(&self, incoming: usize) -> Result<(), Error> {
        let mut count = 0usize;
        let mut bytes = incoming as u64;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let directory = entry.file_name().to_string_lossy().into_owned();
            let Some(name) = directory.strip_prefix("d-") else {
                continue;
            };
            if !valid_id(name) {
                continue;
            }
            for version in self.versions(name)? {
                count += 1;
                let path = self.version_path(&VersionRef { document: name.into(), version })?;
                bytes = bytes.checked_add(fs::metadata(path.join("content"))?.len()).ok_or(Error::Invalid("storage quota"))?;
                if count >= MAX_VERSIONS || bytes > MAX_STORED_CONTENT {
                    return Err(Error::Invalid("storage quota"));
                }
            }
        }
        Ok(())
    }

    pub fn open(root: &Path) -> Result<Self, Error> {
        reject_symlink_ancestors(root)?;
        fs::create_dir_all(root)?;
        let lock = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(root.join("writer.lock"))?;
        lock.try_lock().map_err(|_| Error::Invalid("store is already open"))?;
        if root.join("restore.pending").try_exists()? {
            return Err(Error::Invalid("interrupted restore; preserve this directory and restore to a new destination"));
        }
        let managed = managed::ManagedStore::open(root)?;
        Ok(Self { root: root.to_path_buf(), _lock: lock, managed })
    }

    fn document_path(&self, document: &str) -> Result<PathBuf, Error> {
        if !valid_id(document) {
            return Err(Error::Invalid("document ID"));
        }
        Ok(self.root.join(format!("d-{document}")))
    }

    fn version_path(&self, reference: &VersionRef) -> Result<PathBuf, Error> {
        if reference.version == 0 {
            return Err(Error::Invalid("version must be positive"));
        }
        Ok(self.document_path(&reference.document)?.join(reference.version.to_string()))
    }

    fn versions(&self, document: &str) -> Result<Vec<u64>, Error> {
        let path = self.document_path(document)?;
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut versions = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if let Some(n) = entry.file_name().to_str().and_then(|s| s.parse::<u64>().ok())
                && n > 0
            {
                versions.push(n);
            }
        }
        versions.sort_unstable();
        Ok(versions)
    }

    pub fn download(&self, reference: &VersionRef) -> Result<(Version, Vec<u8>), Error> {
        if self.managed.contains(&reference.document) {
            return self.managed.download(reference);
        }
        let path = self.version_path(reference)?;
        if !path.is_dir() {
            return Err(Error::NotFound);
        }
        let version: Version = serde_json::from_slice(&fs::read(path.join("version.json"))?)?;
        let content = fs::read(path.join("content"))?;
        if version.reference != *reference || digest(&content) != version.sha256 {
            return Err(Error::Integrity);
        }
        Ok((version, content))
    }

    pub fn check_in(&mut self, document: String, parent: Option<u64>, content: Vec<u8>, dependencies: Vec<VersionRef>) -> Result<Version, Error> {
        if self.managed.contains(&document) || document.starts_with("m-") {
            return Err(Error::Forbidden);
        }
        let path = self.document_path(&document)?;
        if content.len() > MAX_CONTENT || dependencies.len() > MAX_DEPENDENCIES {
            return Err(Error::Invalid("size limit"));
        }
        let latest = self.versions(&document)?.last().copied();
        if latest != parent {
            return Err(Error::Conflict);
        }
        self.check_quota(content.len())?;
        let mut unique = BTreeSet::new();
        for dep in &dependencies {
            if dep.document == document || !unique.insert(dep) {
                return Err(Error::Invalid("self or duplicate dependency"));
            }
            self.download(dep)?;
        }
        let number = latest.unwrap_or(0).checked_add(1).ok_or(Error::Invalid("version exhausted"))?;
        let version = Version { reference: VersionRef { document, version: number }, parent, sha256: digest(&content), dependencies };
        fs::create_dir_all(&path)?;
        let staging = path.join("pending");
        // Only this process owns the store lock. A leftover unpublished stage is disposable.
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir(&staging)?;
        write_synced(&staging.join("content"), &content)?;
        write_synced(&staging.join("version.json"), &serde_json::to_vec(&version)?)?;
        fs::rename(&staging, self.version_path(&version.reference)?)?;
        Ok(version)
    }

    pub fn where_used(&self, reference: &VersionRef) -> Result<Vec<VersionRef>, Error> {
        self.download(reference)?;
        let mut result = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let directory = entry.file_name().to_string_lossy().into_owned();
            let Some(name) = directory.strip_prefix("d-") else {
                continue;
            };
            if !valid_id(name) {
                continue;
            }
            for version in self.versions(name)? {
                let reference_candidate = VersionRef { document: name.to_owned(), version };
                let (metadata, _) = self.download(&reference_candidate)?;
                if metadata.dependencies.contains(reference) {
                    result.push(reference_candidate);
                }
            }
        }
        result.extend(self.managed.where_used(reference));
        result.sort();
        result.dedup();
        Ok(result)
    }

    pub fn execute(&mut self, request: Request) -> Response {
        self.execute_as(request, &managed::Principal { id: "owner".into(), role: managed::Role::Administrator })
    }

    /// Secure transports require configured accounts and never the loopback owner.
    pub(crate) fn has_configured_principals(&self) -> bool {
        self.managed.has_configured_principals()
    }

    pub(crate) fn execute_configured(&mut self, request: Request, bearer: &str) -> Response {
        match self.managed.authenticate(bearer) {
            Some(principal) => self.execute_as(request, &principal),
            None => Response::Error { code: "unauthorized".into() },
        }
    }

    pub fn execute_as(&mut self, request: Request, principal: &managed::Principal) -> Response {
        let result = match request {
            Request::ManagedCatalog => Ok(Response::ManagedCatalog { catalog: self.managed.catalog() }),
            Request::ManagedCreate { logical_path } => self.managed.create(principal, logical_path).map(|entry| Response::ManagedDocument { entry }),
            Request::ManagedCheckout { document, workspace, machine } => {
                self.managed.checkout(principal, &document, workspace, machine).map(|grant| Response::ManagedCheckout { grant })
            }
            Request::ManagedCancelCheckout { document, workspace, machine, checkout_token } => {
                self.managed.cancel(principal, &document, &workspace, &machine, &checkout_token).map(|entry| Response::ManagedDocument { entry })
            }
            Request::ManagedCheckIn { input } => self.managed.check_in(principal, input).map(|version| Response::ManagedCheckedIn { version }),
            Request::ManagedHistory { document } => self.managed.history(&document).map(|versions| Response::ManagedHistory { versions }),
            Request::List { document } => self.list(document.as_deref()).map(|versions| Response::Listed { versions }),
            Request::CheckIn { document, parent, content, dependencies } => {
                if principal.role == managed::Role::Viewer {
                    Err(Error::Forbidden)
                } else {
                    self.check_in(document, parent, content, dependencies).map(|version| Response::CheckedIn { version })
                }
            }
            Request::Download { reference } => self.download(&reference).map(|(version, content)| Response::Downloaded { version, content }),
            Request::WhereUsed { reference } => self.where_used(&reference).map(|versions| Response::WhereUsed { versions }),
        };
        result.unwrap_or_else(|error| Response::Error {
            code: match error {
                Error::Forbidden => "forbidden",
                Error::Conflict => "conflict",
                Error::NotFound => "not_found",
                Error::Invalid(_) => "invalid_request",
                Error::Integrity => "integrity",
                _ => "storage_error",
            }
            .into(),
        })
    }
}

// Fail closed for pre-existing symlink/junction store paths. The store directory
// remains trusted local state: this does not defend against concurrent host tampering.
pub(crate) fn reject_symlink_ancestors(path: &Path) -> Result<(), Error> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Err(Error::Invalid("symlink store path")),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn write_synced(path: &Path, content: &[u8]) -> Result<(), Error> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(content)?;
    file.sync_all()?;
    Ok(())
}

/// Read one bounded frame; callers must provide a reader with appropriate time limits.
pub fn read_frame(reader: impl Read) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    BufReader::new(reader.take(MAX_FRAME + 1)).read_until(b'\n', &mut bytes)?;
    if bytes.len() as u64 > MAX_FRAME || bytes.last() != Some(&b'\n') {
        return Err(Error::Invalid("frame size or terminator"));
    }
    Ok(bytes)
}

/// An absolute frame budget is recomputed before every underlying socket read.
pub fn read_socket_frame(stream: &TcpStream, budget: Duration) -> Result<Vec<u8>, Error> {
    let started = Instant::now();
    let mut stream = stream;
    read_budgeted_frame(
        |buffer, remaining| {
            stream.set_read_timeout(Some(remaining))?;
            stream.read(buffer)
        },
        || started.elapsed(),
        budget,
    )
}

// Clock and I/O seams keep the repeated-read regression independent of thread scheduling.
fn read_budgeted_frame<R, C>(reader: R, elapsed: C, budget: Duration) -> Result<Vec<u8>, Error>
where
    R: FnMut(&mut [u8], Duration) -> std::io::Result<usize>,
    C: FnMut() -> Duration,
{
    struct BudgetReader<R, C> {
        reader: R,
        elapsed: C,
        budget: Duration,
    }
    impl<R, C> Read for BudgetReader<R, C>
    where
        R: FnMut(&mut [u8], Duration) -> std::io::Result<usize>,
        C: FnMut() -> Duration,
    {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let remaining = self
                .budget
                .checked_sub((self.elapsed)())
                .filter(|duration| !duration.is_zero())
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::TimedOut, "frame deadline exceeded"))?;
            (self.reader)(buffer, remaining)
        }
    }
    read_frame(BudgetReader { reader, elapsed, budget })
}

pub fn serve_connection(store: &mut Store, mut stream: TcpStream, token: &AuthToken) -> Result<(), Error> {
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let response =
        match read_socket_frame(&stream, Duration::from_secs(5)).and_then(|bytes| Ok(serde_json::from_slice::<AuthenticatedRequest>(&bytes)?)) {
            Ok(envelope) if envelope.protocol != PROTOCOL => Response::Error { code: "unsupported_protocol".into() },
            Ok(envelope) => {
                let principal = if token.matches(&envelope.token) {
                    Some(managed::Principal { id: "owner".into(), role: managed::Role::Administrator })
                } else {
                    store.managed.authenticate(&envelope.token)
                };
                match principal {
                    Some(principal) => store.execute_as(envelope.request, &principal),
                    None => Response::Error { code: "unauthorized".into() },
                }
            }
            Err(_) => Response::Error { code: "invalid_request".into() },
        };
    let bytes = serde_json::to_vec(&response)?;
    write_socket_frame(&mut stream, &bytes, Duration::from_secs(5))?;
    Ok(())
}

/// Serialize before entering this function so tiny JSON writes cannot extend a connection.
pub fn write_socket_frame(stream: &mut TcpStream, bytes: &[u8], budget: Duration) -> Result<(), Error> {
    let started = Instant::now();
    write_budgeted_frame(
        |bytes, remaining| {
            stream.set_write_timeout(Some(remaining))?;
            stream.write(bytes)
        },
        || started.elapsed(),
        bytes,
        budget,
    )
}

fn write_budgeted_frame<W, C>(writer: W, elapsed: C, bytes: &[u8], budget: Duration) -> Result<(), Error>
where
    W: FnMut(&[u8], Duration) -> std::io::Result<usize>,
    C: FnMut() -> Duration,
{
    if bytes.len() as u64 >= MAX_FRAME {
        return Err(Error::Invalid("response frame size"));
    }
    struct BudgetWriter<W, C> {
        writer: W,
        elapsed: C,
        budget: Duration,
    }
    impl<W, C> Write for BudgetWriter<W, C>
    where
        W: FnMut(&[u8], Duration) -> std::io::Result<usize>,
        C: FnMut() -> Duration,
    {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let remaining = self
                .budget
                .checked_sub((self.elapsed)())
                .filter(|duration| !duration.is_zero())
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::TimedOut, "response deadline exceeded"))?;
            (self.writer)(bytes, remaining)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = BudgetWriter { writer, elapsed, budget };
    writer.write_all(bytes)?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_dispatch_requires_enrollment_and_rejects_unregistered_owner() {
        let root = std::env::temp_dir().join(format!("solidcraft-configured-auth-{}", AuthToken::generate().unwrap()));
        let unknown_owner = AuthToken::generate().unwrap();
        let mut store = Store::open(&root).unwrap();
        assert!(!store.has_configured_principals());
        assert!(matches!(
            store.execute_configured(Request::ManagedCatalog, &unknown_owner),
            Response::Error { code } if code == "unauthorized"
        ));
        drop(store);
        let (_, enrolled) = bootstrap::initialize(&root, "administrator").unwrap();
        let enrolled = enrolled.into_bearer();
        let mut store = Store::open(&root).unwrap();
        assert!(store.has_configured_principals());
        assert!(matches!(store.execute_configured(Request::ManagedCatalog, &enrolled), Response::ManagedCatalog { .. }));
        assert!(matches!(
            store.execute_configured(Request::ManagedCatalog, &unknown_owner),
            Response::Error { code } if code == "unauthorized"
        ));
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_write_retries_with_remaining_budget_and_preserves_bytes() {
        let elapsed = std::cell::Cell::new(Duration::ZERO);
        let mut attempts = 0;
        let mut output = Vec::new();
        let mut budgets = Vec::new();
        let result = write_budgeted_frame(
            |bytes, remaining| {
                attempts += 1;
                budgets.push(remaining.as_millis());
                elapsed.set(elapsed.get() + Duration::from_millis(10));
                if attempts == 2 {
                    return Err(std::io::Error::from(std::io::ErrorKind::Interrupted));
                }
                if let Some(byte) = bytes.first() {
                    output.push(*byte);
                    Ok(1)
                } else {
                    Err(std::io::Error::other("empty write"))
                }
            },
            || elapsed.get(),
            b"ab",
            Duration::from_millis(100),
        );
        assert!(result.is_ok());
        assert_eq!(attempts, 4);
        assert_eq!(output, b"ab\n");
        assert_eq!(budgets, vec![100, 90, 80, 70]);
    }

    #[test]
    fn repeated_partial_writes_do_not_reset_the_response_deadline() {
        let elapsed = std::cell::Cell::new(Duration::ZERO);
        let mut written = 0;
        let mut budgets = Vec::new();
        let result = write_budgeted_frame(
            |bytes, remaining| {
                assert!(!bytes.is_empty());
                written += 1;
                budgets.push(remaining.as_millis());
                elapsed.set(elapsed.get() + Duration::from_millis(20));
                Ok(1)
            },
            || elapsed.get(),
            b"123456",
            Duration::from_millis(100),
        );
        assert!(matches!(result, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::TimedOut));
        assert_eq!(written, 5);
        assert_eq!(budgets, vec![100, 80, 60, 40, 20]);
    }

    #[test]
    fn repeated_reads_do_not_reset_the_frame_deadline() {
        let elapsed = std::cell::Cell::new(Duration::ZERO);
        let mut reads = 0;
        let mut budgets = Vec::new();
        let result = read_budgeted_frame(
            |buffer, remaining| {
                let byte = buffer.first_mut().ok_or_else(|| std::io::Error::other("empty read buffer"))?;
                *byte = b' ';
                reads += 1;
                budgets.push(remaining.as_millis());
                elapsed.set(elapsed.get() + Duration::from_millis(20));
                Ok(1)
            },
            || elapsed.get(),
            Duration::from_millis(100),
        );
        assert!(matches!(result, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::TimedOut));
        assert_eq!(reads, 5);
        assert_eq!(budgets, vec![100, 80, 60, 40, 20]);
    }
}
