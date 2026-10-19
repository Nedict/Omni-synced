//! A private, single-process managed working folder. Server grants remain authoritative.
//!
//! Journal records are immutable. An interrupted mutation leaves `pending` and recoverable
//! content backups; opening then fails closed rather than guessing which server operation won.
use crate::{Error, MAX_CONTENT, digest, valid_id};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const FORMAT: &str = "solidcraft-managed-workspace/1";
const MAX_MANIFEST: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub relative_path: String,
    pub base_version: u64,
    pub base_sha256: String,
    /// Local convenience state, never proof of a server grant.
    pub checked_out: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub workspace_id: String,
    pub server_id: String,
    pub generation: u64,
    pub documents: BTreeMap<String, Entry>,
}

/// Holding this object excludes another cooperating local writer with an OS file lock.
pub struct Workspace {
    root: PathBuf,
    metadata: PathBuf,
    manifest: Manifest,
    _lock: File,
}

impl Workspace {
    pub fn create(root: &Path, server_id: &str) -> Result<Self, Error> {
        if !valid_id(server_id) {
            return Err(Error::Invalid("invalid server identity"));
        }
        // Initialization is explicit; existing unmanaged files are never touched.
        match fs::create_dir(root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                reject_link(root)?;
                if !fs::metadata(root)?.is_dir() {
                    return Err(Error::Invalid("workspace root is not a directory"));
                }
            }
            Err(error) => return Err(error.into()),
        }
        let root = fs::canonicalize(root)?;
        let metadata = root.join(".solidcraft-pdm");
        fs::create_dir(&metadata)?;
        let lock = workspace_lock(&metadata)?;
        let mut entropy = [0u8; 16];
        getrandom::fill(&mut entropy).map_err(|_| Error::Invalid("OS randomness unavailable"))?;
        let workspace_id = entropy.iter().map(|b| format!("{b:02x}")).collect();
        let manifest = Manifest { format: FORMAT.into(), workspace_id, server_id: server_id.into(), generation: 0, documents: BTreeMap::new() };
        let workspace = Self { root, metadata, manifest, _lock: lock };
        workspace.write_manifest(&workspace.manifest)?;
        Ok(workspace)
    }

    pub fn open(root: &Path, server_id: &str) -> Result<Self, Error> {
        Self::open_mode(root, server_id, false)
    }
    /// Explicit recovery opening never adopts the possibly uncommitted newest state.
    pub fn open_for_recovery(root: &Path, server_id: &str) -> Result<Self, Error> {
        Self::open_mode(root, server_id, true)
    }
    fn open_mode(root: &Path, server_id: &str, recovery: bool) -> Result<Self, Error> {
        reject_link(root)?;
        let root = fs::canonicalize(root)?;
        let metadata = root.join(".solidcraft-pdm");
        reject_link(&metadata)?;
        let lock = workspace_lock(&metadata)?;
        let loaded = (|| {
            if metadata.join("pending").try_exists()? {
                if !recovery {
                    return Err(Error::Invalid("workspace recovery required"));
                }
                let pending = read_pending(&metadata)?;
                if pending.manifest.server_id != server_id || pending.manifest.format != FORMAT {
                    return Err(Error::Integrity);
                }
                return Ok(pending.manifest);
            }
            let mut generations = Vec::new();
            for item in fs::read_dir(&metadata)? {
                let item = item?;
                let name = item.file_name();
                if let Some(number) = name.to_str().and_then(|s| s.strip_suffix(".json")).and_then(|s| s.parse::<u64>().ok()) {
                    generations.push(number);
                }
            }
            let generation = generations.into_iter().max().ok_or(Error::Invalid("workspace manifest missing; recovery required"))?;
            let path = metadata.join(format!("{generation}.json"));
            reject_link(&path)?;
            let mut bytes = Vec::new();
            File::open(path)?.take(MAX_MANIFEST + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_MANIFEST {
                return Err(Error::Invalid("workspace manifest too large"));
            }
            let manifest: Manifest = serde_json::from_slice(&bytes)?;
            if manifest.format != FORMAT || manifest.server_id != server_id || manifest.generation != generation || !valid_id(&manifest.workspace_id)
            {
                return Err(Error::Integrity);
            }
            let mut paths = std::collections::BTreeSet::new();
            for (document, entry) in &manifest.documents {
                validate_entry(document, entry)?;
                if !paths.insert(entry.relative_path.to_ascii_lowercase()) {
                    return Err(Error::Integrity);
                }
            }
            Ok(manifest)
        })();
        match loaded {
            Ok(manifest) => Ok(Self { root, metadata, manifest, _lock: lock }),
            Err(error) => {
                drop(lock);
                Err(error)
            }
        }
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn path(&self, document: &str) -> Result<PathBuf, Error> {
        self.safe_path(&self.entry(document)?.relative_path, false)
    }
    pub fn local_sha256(&self, document: &str) -> Result<String, Error> {
        hash_file(&self.path(document)?)
    }

    /// Adopt the same saved path only after a successful server publication.
    pub fn adopt(&mut self, document: &str, relative: &str, version: u64, sha256: &str) -> Result<(), Error> {
        let entry = self.new_entry(document, relative, version, sha256)?;
        if let Some(old) = self.manifest.documents.get(document) {
            if old != &entry {
                return Err(Error::Conflict);
            }
            if self.local_sha256(document)? == sha256 {
                return Ok(());
            }
            return Err(Error::Conflict);
        }
        let path = self.safe_path(relative, false)?;
        if hash_file(&path)? != sha256 {
            return Err(Error::Conflict);
        }
        self.begin(Some(&entry.relative_path))?;
        set_writable(&path, false)?;
        let mut next = self.manifest.clone();
        next.documents.insert(document.into(), entry);
        self.finish(next)
    }

    /// Download to a stable normal working path. Existing dirty/checked-out files are protected.
    pub fn install(&mut self, document: &str, relative: &str, version: u64, sha256: &str, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > MAX_CONTENT || digest(bytes) != sha256 {
            return Err(Error::Integrity);
        }
        let entry = self.new_entry(document, relative, version, sha256)?;
        let path = self.safe_path(relative, true)?;
        if let Some(old) = self.manifest.documents.get(document) {
            if old.checked_out || old.relative_path != relative || (path.try_exists()? && hash_file(&path)? != old.base_sha256) {
                return Err(Error::Conflict);
            }
        } else if path.try_exists()? {
            return Err(Error::Conflict);
        }
        self.replace(document, path, entry, bytes)
    }

    /// Call only after obtaining/confirming the exclusive server grant for this workspace.
    pub fn checkout(&mut self, document: &str) -> Result<(), Error> {
        let mut entry = self.entry(document)?.clone();
        let path = self.path(document)?;
        if !entry.checked_out && hash_file(&path)? != entry.base_sha256 {
            return Err(Error::Conflict);
        }
        self.begin(Some(&entry.relative_path))?;
        set_writable(&path, true)?;
        entry.checked_out = true;
        let mut next = self.manifest.clone();
        next.documents.insert(document.into(), entry);
        self.finish(next)
    }

    /// A changed file after upload stays writable and is never falsely marked synchronized.
    pub fn record_checkin(&mut self, document: &str, version: u64, sha256: &str) -> Result<(), Error> {
        let old = self.entry(document)?.clone();
        let mut entry = self.new_entry(document, &old.relative_path, version, sha256)?;
        let path = self.path(document)?;
        if !old.checked_out || hash_file(&path)? != sha256 {
            return Err(Error::Conflict);
        }
        self.begin(Some(&entry.relative_path))?;
        set_writable(&path, false)?;
        if hash_file(&path)? != sha256 {
            set_writable(&path, true)?;
            return Err(Error::Conflict);
        }
        entry.checked_out = false;
        let mut next = self.manifest.clone();
        next.documents.insert(document.into(), entry);
        self.finish(next)
    }

    /// Explicit destructive intent is mandatory; the old bytes remain in a recovery backup.
    /// Caller verifies its server grant and releases it only after this operation succeeds.
    /// Local checkout state may lag an acquired grant or a lost cancellation response.
    pub fn undo_checkout(&mut self, document: &str, bytes: &[u8], discard_changes: bool) -> Result<(), Error> {
        if !discard_changes {
            return Err(Error::Invalid("explicit discard confirmation required"));
        }
        let mut entry = self.entry(document)?.clone();
        if bytes.len() > MAX_CONTENT || digest(bytes) != entry.base_sha256 {
            return Err(Error::Integrity);
        }
        entry.checked_out = false;
        self.replace(document, self.path(document)?, entry, bytes)
    }

    /// Roll back LOCAL state only after the caller reconciles the server's grant/version.
    /// Edited/new bytes are retained in metadata recovery files; server history is untouched.
    pub fn recover_pending(&mut self, server_reconciled: bool) -> Result<(), Error> {
        if !server_reconciled {
            return Err(Error::Invalid("server reconciliation required before local recovery"));
        }
        let pending = read_pending(&self.metadata)?;
        if pending.manifest.workspace_id != self.manifest.workspace_id || pending.manifest.server_id != self.manifest.server_id {
            return Err(Error::Integrity);
        }
        if let Some(relative) = pending.relative_path.as_deref() {
            let path = self.safe_path(relative, false)?;
            let backup = self.metadata.join(format!("backup-{}", pending.manifest.generation));
            let previous = pending.manifest.documents.values().find(|e| e.relative_path == relative);
            if backup.try_exists()? || previous.is_none() {
                if path.try_exists()? {
                    let bytes = read_content(&path)?;
                    new_file(&self.metadata.join(format!("recovery-current-{}", pending.manifest.generation)), &bytes)?;
                    set_writable(&path, true)?;
                    fs::remove_file(&path)?;
                }
                if backup.try_exists()? {
                    let bytes = read_content(&backup)?;
                    new_file(&path, &bytes)?;
                }
            }
            if let Some(previous) = previous {
                set_writable(&path, previous.checked_out)?;
            }
        }
        let next = pending.manifest.generation.checked_add(1).ok_or(Error::Integrity)?;
        for name in [format!("{next}.json"), format!("{next}.pending"), "content.pending".into()] {
            let path = self.metadata.join(&name);
            if path.try_exists()? {
                reject_link(&path)?;
                fs::rename(&path, self.metadata.join(format!("recovery-{}-{name}", pending.manifest.generation)))?;
            }
        }
        self.manifest = pending.manifest;
        self.finish(self.manifest.clone())
    }
    fn entry(&self, document: &str) -> Result<&Entry, Error> {
        self.manifest.documents.get(document).ok_or(Error::NotFound)
    }
    fn new_entry(&self, document: &str, relative: &str, version: u64, sha256: &str) -> Result<Entry, Error> {
        let entry = Entry { relative_path: relative.into(), base_version: version, base_sha256: sha256.into(), checked_out: false };
        validate_entry(document, &entry)?;
        if self.manifest.documents.get(document).is_some_and(|old| old.relative_path != relative) {
            return Err(Error::Conflict);
        }
        if self.manifest.documents.iter().any(|(id, e)| id != document && e.relative_path.eq_ignore_ascii_case(relative)) {
            return Err(Error::Conflict);
        }
        Ok(entry)
    }
    fn safe_path(&self, relative: &str, create_parents: bool) -> Result<PathBuf, Error> {
        validate_relative(relative)?;
        let mut path = self.root.clone();
        let components: Vec<_> = relative.split('/').collect();
        for (index, component) in components.iter().enumerate() {
            path.push(component);
            match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() || is_reparse(&metadata) {
                        return Err(Error::Invalid("workspace links are forbidden"));
                    }
                    if index + 1 < components.len() && !metadata.is_dir() {
                        return Err(Error::Invalid("workspace parent is not a directory"));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if create_parents && index + 1 < components.len() {
                        fs::create_dir(&path)?;
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(path)
    }
    fn begin(&self, relative: Option<&str>) -> Result<(), Error> {
        let pending = Pending { manifest: self.manifest.clone(), relative_path: relative.map(str::to_owned) };
        new_file(&self.metadata.join("pending"), &serde_json::to_vec(&pending)?)?;
        Ok(())
    }
    fn finish(&mut self, mut next: Manifest) -> Result<(), Error> {
        next.generation = self.manifest.generation.checked_add(1).ok_or(Error::Invalid("workspace generation limit"))?;
        self.write_manifest(&next)?;
        self.manifest = next;
        fs::remove_file(self.metadata.join("pending"))?;
        Ok(())
    }
    fn write_manifest(&self, manifest: &Manifest) -> Result<(), Error> {
        let bytes = serde_json::to_vec_pretty(manifest)?;
        if bytes.len() as u64 > MAX_MANIFEST {
            return Err(Error::Invalid("workspace manifest too large"));
        }
        let temporary = self.metadata.join(format!("{}.pending", manifest.generation));
        new_file(&temporary, &bytes)?;
        fs::rename(temporary, self.metadata.join(format!("{}.json", manifest.generation)))?;
        sync_directory(&self.metadata)
    }
    fn replace(&mut self, document: &str, path: PathBuf, entry: Entry, bytes: &[u8]) -> Result<(), Error> {
        self.begin(Some(&entry.relative_path))?;
        let staged = self.metadata.join("content.pending");
        new_file(&staged, bytes)?;
        if path.try_exists()? {
            let backup = self.metadata.join(format!("backup-{}", self.manifest.generation));
            // Copy first: preserve the exact previous bytes even if replacement fails.
            let old = read_content(&path)?;
            new_file(&backup, &old)?;
            set_writable(&path, true)?;
            fs::remove_file(&path)?;
        }
        fs::rename(staged, &path)?;
        set_writable(&path, false)?;
        if let Some(parent) = path.parent() {
            sync_directory(parent)?;
        }
        let mut next = self.manifest.clone();
        next.documents.insert(document.into(), entry);
        self.finish(next)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    manifest: Manifest,
    relative_path: Option<String>,
}
fn read_pending(metadata: &Path) -> Result<Pending, Error> {
    let path = metadata.join("pending");
    reject_link(&path)?;
    let mut bytes = Vec::new();
    File::open(path)?.take(MAX_MANIFEST + 2048).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST + 1024 {
        return Err(Error::Integrity);
    }
    let pending: Pending = serde_json::from_slice(&bytes)?;
    for (id, entry) in &pending.manifest.documents {
        validate_entry(id, entry)?;
    }
    if let Some(relative) = &pending.relative_path {
        validate_relative(relative)?;
    }
    Ok(pending)
}
fn workspace_lock(metadata: &Path) -> Result<File, Error> {
    let path = metadata.join("lock");
    if path.try_exists()? {
        reject_link(&path)?;
    }
    let lock = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)?;
    lock.try_lock().map_err(|_| Error::Invalid("workspace already open"))?;
    Ok(lock)
}
fn validate_entry(document: &str, entry: &Entry) -> Result<(), Error> {
    if !valid_id(document)
        || entry.base_version == 0
        || entry.base_sha256.len() != 64
        || !entry.base_sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(Error::Invalid("invalid managed document metadata"));
    }
    validate_relative(&entry.relative_path)
}
fn validate_relative(relative: &str) -> Result<(), Error> {
    if relative.len() > 1024 || relative.is_empty() || relative.contains('\\') {
        return Err(Error::Invalid("invalid workspace path"));
    }
    for component in relative.split('/') {
        let stem = component.split('.').next().unwrap_or_default().to_ascii_uppercase();
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.len() > 200
            || component.ends_with(['.', ' '])
            || component.chars().any(|c| c.is_control() || "<>:\"|?*".contains(c))
            || component.eq_ignore_ascii_case(".solidcraft-pdm")
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes().last().is_some_and(|b| b.is_ascii_digit()))
        {
            return Err(Error::Invalid("invalid workspace path"));
        }
    }
    Ok(())
}
fn reject_link(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return Err(Error::Invalid("workspace links are forbidden"));
    }
    Ok(())
}
#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_reparse(_: &fs::Metadata) -> bool {
    false
}
fn set_writable(path: &Path, writable: bool) -> Result<(), Error> {
    let mut permissions = fs::metadata(path)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = permissions.mode();
        permissions.set_mode(if writable { mode | 0o200 } else { mode & !0o222 });
    }
    #[cfg(not(unix))]
    permissions.set_readonly(!writable);
    fs::set_permissions(path, permissions)?;
    Ok(())
}
fn new_file(path: &Path, bytes: &[u8]) -> Result<File, Error> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(file)
}
fn read_content(path: &Path) -> Result<Vec<u8>, Error> {
    reject_link(path)?;
    if !fs::metadata(path)?.is_file() {
        return Err(Error::Invalid("workspace content is not a regular file"));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(MAX_CONTENT as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_CONTENT {
        return Err(Error::Invalid("workspace content too large"));
    }
    Ok(bytes)
}
fn hash_file(path: &Path) -> Result<String, Error> {
    Ok(digest(&read_content(path)?))
}
#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), Error> {
    File::open(path)?.sync_all()?;
    Ok(())
}
#[cfg(not(unix))]
fn sync_directory(_: &Path) -> Result<(), Error> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temporary(PathBuf);
    impl Temporary {
        fn new() -> Result<Self, Error> {
            let mut entropy = [0; 16];
            getrandom::fill(&mut entropy).map_err(|_| Error::Invalid("test entropy"))?;
            let name: String = entropy.iter().map(|b| format!("{b:02x}")).collect();
            Ok(Self(std::env::temp_dir().join(format!("solidcraft-managed-{name}"))))
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            fn writable_tree(path: &Path) {
                if let Ok(children) = fs::read_dir(path) {
                    for child in children.flatten() {
                        let path = child.path();
                        if path.is_dir() {
                            writable_tree(&path);
                        } else {
                            let _ = set_writable(&path, true);
                        }
                    }
                }
            }
            writable_tree(&self.0);
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn stable_path_round_trip_and_checkout_retry_preserves_changes() -> Result<(), Error> {
        let temp = Temporary::new()?;
        let mut workspace = Workspace::create(&temp.0, "server")?;
        let id = workspace.manifest().workspace_id.clone();
        workspace.install("part", "parts/toy.solidcraft", 1, &digest(b"v1"), b"v1")?;
        let path = workspace.path("part")?;
        assert!(fs::metadata(&path)?.permissions().readonly());
        workspace.checkout("part")?;
        fs::write(&path, b"v2")?;
        workspace.checkout("part")?;
        assert_eq!(fs::read(&path)?, b"v2");
        assert!(workspace.record_checkin("part", 2, &digest(b"other")).is_err());
        assert!(!fs::metadata(&path)?.permissions().readonly());
        workspace.record_checkin("part", 2, &digest(b"v2"))?;
        assert_eq!(workspace.path("part")?, path);
        drop(workspace);
        let workspace = Workspace::open(&temp.0, "server")?;
        assert_eq!(workspace.manifest().workspace_id, id);
        assert_eq!(workspace.manifest().documents.get("part").map(|e| e.base_version), Some(2));
        Ok(())
    }
    #[test]
    fn dirty_download_and_unconfirmed_undo_never_overwrite() -> Result<(), Error> {
        let temp = Temporary::new()?;
        let mut workspace = Workspace::create(&temp.0, "server")?;
        workspace.install("part", "toy.solidcraft", 1, &digest(b"base"), b"base")?;
        workspace.checkout("part")?;
        let path = workspace.path("part")?;
        fs::write(&path, b"dirty")?;
        assert!(workspace.install("part", "toy.solidcraft", 2, &digest(b"next"), b"next").is_err());
        assert!(workspace.undo_checkout("part", b"base", false).is_err());
        assert_eq!(fs::read(&path)?, b"dirty");
        let generation = workspace.manifest().generation;
        workspace.undo_checkout("part", b"base", true)?;
        assert_eq!(fs::read(&path)?, b"base");
        assert_eq!(fs::read(workspace.metadata.join(format!("backup-{generation}")))?, b"dirty");
        Ok(())
    }
    #[test]
    fn undo_reconciles_server_grant_when_local_checkout_was_not_recorded() -> Result<(), Error> {
        let temp = Temporary::new()?;
        let mut workspace = Workspace::create(&temp.0, "server")?;
        workspace.install("part", "toy.solidcraft", 1, &digest(b"base"), b"base")?;
        let path = workspace.path("part")?;
        // An external save bypassed the readonly convenience bit before local grant recording.
        set_writable(&path, true)?;
        fs::write(&path, b"external edits")?;
        set_writable(&path, false)?;
        assert!(workspace.undo_checkout("part", b"base", false).is_err());
        assert!(workspace.undo_checkout("part", b"unverified", true).is_err());
        assert_eq!(fs::read(&path)?, b"external edits");
        let generation = workspace.manifest().generation;
        workspace.undo_checkout("part", b"base", true)?;
        assert_eq!(fs::read(&path)?, b"base");
        assert!(fs::metadata(&path)?.permissions().readonly());
        assert_eq!(fs::read(workspace.metadata.join(format!("backup-{generation}")))?, b"external edits");
        assert!(!workspace.entry("part")?.checked_out);
        Ok(())
    }
    #[test]
    fn existing_root_and_explicit_recovery_preserve_user_bytes() -> Result<(), Error> {
        let temp = Temporary::new()?;
        fs::create_dir(&temp.0)?;
        fs::write(temp.0.join("saved.solidcraft"), b"saved")?;
        let mut workspace = Workspace::create(&temp.0, "server")?;
        assert!(workspace.install("part", "saved.solidcraft", 1, &digest(b"other"), b"other").is_err());
        workspace.adopt("part", "saved.solidcraft", 1, &digest(b"saved"))?;
        workspace.checkout("part")?;
        let generation = workspace.manifest().generation;
        workspace.begin(Some("saved.solidcraft"))?;
        new_file(&workspace.metadata.join(format!("backup-{generation}")), b"saved")?;
        fs::write(temp.0.join("saved.solidcraft"), b"interrupted")?;
        drop(workspace);
        assert!(Workspace::open(&temp.0, "server").is_err());
        let mut workspace = Workspace::open_for_recovery(&temp.0, "server")?;
        assert!(workspace.recover_pending(false).is_err());
        workspace.recover_pending(true)?;
        assert_eq!(fs::read(temp.0.join("saved.solidcraft"))?, b"saved");
        assert_eq!(fs::read(workspace.metadata.join(format!("recovery-current-{generation}")))?, b"interrupted");
        drop(workspace);
        Workspace::open(&temp.0, "server")?;
        Ok(())
    }
    #[test]
    fn adoption_verifies_existing_bytes_and_server_binding() -> Result<(), Error> {
        let temp = Temporary::new()?;
        let mut workspace = Workspace::create(&temp.0, "server")?;
        let path = temp.0.join("saved.solidcraft");
        fs::write(&path, b"saved")?;
        assert!(workspace.adopt("part", "saved.solidcraft", 1, &digest(b"different")).is_err());
        workspace.adopt("part", "saved.solidcraft", 1, &digest(b"saved"))?;
        assert_eq!(workspace.path("part")?, fs::canonicalize(&path)?);
        drop(workspace);
        assert!(Workspace::open(&temp.0, "another-server").is_err());
        let workspace = Workspace::open(&temp.0, "server")?;
        drop(workspace);
        Ok(())
    }
    #[test]
    fn missing_manifest_and_interrupted_mutation_fail_closed() -> Result<(), Error> {
        let temp = Temporary::new()?;
        let workspace = Workspace::create(&temp.0, "server")?;
        drop(workspace);
        fs::remove_file(temp.0.join(".solidcraft-pdm/0.json"))?;
        assert!(Workspace::open(&temp.0, "server").is_err());
        fs::write(temp.0.join("keep"), b"user")?;
        assert!(Workspace::create(&temp.0, "server").is_err());
        assert_eq!(fs::read(temp.0.join("keep"))?, b"user");
        let other = Temporary::new()?;
        let workspace = Workspace::create(&other.0, "server")?;
        workspace.begin(None)?;
        drop(workspace);
        assert!(Workspace::open(&other.0, "server").is_err());
        Ok(())
    }
    #[test]
    fn hostile_paths_and_case_aliases_are_rejected() -> Result<(), Error> {
        for path in
            ["../escape", "/root", "a//b", "C:/file", "a\\b", "x/../b", "NUL.txt", "COM1", ".solidcraft-pdm/state", "foo.", "foo ", "bad\0name"]
        {
            assert!(validate_relative(path).is_err(), "accepted {path:?}");
        }
        let temp = Temporary::new()?;
        let mut workspace = Workspace::create(&temp.0, "server")?;
        workspace.install("part", "Toy.solidcraft", 1, &digest(b"base"), b"base")?;
        assert!(workspace.install("other", "toy.solidcraft", 1, &digest(b"base"), b"base").is_err());
        assert!(Workspace::open(&temp.0, "server").is_err());
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn symlink_parent_cannot_escape_root() -> Result<(), Error> {
        let temp = Temporary::new()?;
        let outside = Temporary::new()?;
        fs::create_dir(&outside.0)?;
        let mut workspace = Workspace::create(&temp.0, "server")?;
        std::os::unix::fs::symlink(&outside.0, temp.0.join("linked"))?;
        assert!(workspace.install("part", "linked/toy.solidcraft", 1, &digest(b"x"), b"x").is_err());
        assert!(!outside.0.join("toy.solidcraft").exists());
        Ok(())
    }
}
