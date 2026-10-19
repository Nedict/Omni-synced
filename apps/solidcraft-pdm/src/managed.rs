//! Managed catalog, fenced exclusive checkout and atomic receipt-bearing check-in.
//! Credentials are resolved by the server; request fields never choose a principal.
use crate::blob_store::{BlobStore, FsBlobStore};
use crate::{Error, MAX_CONTENT, MAX_DEPENDENCIES, MAX_STORED_CONTENT, MAX_VERSIONS, Version, VersionRef, digest};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_STATE: usize = 4 * 1024 * 1024;
const MAX_GENERATIONS: u64 = 4096;
const MAX_METADATA_STORAGE: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Viewer,
    Contributor,
    Administrator,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub id: String,
    pub role: Role,
}

/// This file is trusted server configuration, never supplied in a network request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalCredential {
    pub principal: Principal,
    pub token_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkout {
    pub owner: String,
    pub workspace: String,
    pub machine: String,
    pub checked_out_unix_ms: u64,
    pub base_version: Option<u64>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutGrant {
    pub entry: CatalogEntry,
    pub token: String,
}

impl std::fmt::Debug for CheckoutGrant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CheckoutGrant").field("entry", &self.entry).field("token", &"[redacted]").finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedCatalog {
    pub server_id: String,
    pub entries: Vec<CatalogEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    pub document: String,
    pub logical_path: String,
    pub head: Option<u64>,
    #[serde(default)]
    pub head_checked_in_unix_ms: Option<u64>,
    #[serde(default)]
    pub head_author: Option<String>,
    pub checkout: Option<Checkout>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedCheckIn {
    pub document: String,
    pub workspace: String,
    pub machine: String,
    pub checkout_token: String,
    pub request_id: String,
    pub parent: Option<u64>,
    pub content: Vec<u8>,
    pub dependencies: Vec<VersionRef>,
    pub comment: String,
}

impl std::fmt::Debug for ManagedCheckIn {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagedCheckIn")
            .field("document", &self.document)
            .field("workspace", &self.workspace)
            .field("machine", &self.machine)
            .field("checkout_token", &"[redacted]")
            .field("request_id", &self.request_id)
            .field("parent", &self.parent)
            .field("content_bytes", &self.content.len())
            .field("dependencies", &self.dependencies)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedVersion {
    pub version: Version,
    pub author: String,
    pub checked_in_unix_ms: u64,
    pub comment: String,
    pub request_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    owner: String,
    fingerprint: String,
    version: ManagedVersion,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    server_id: String,
    checkout_tokens: BTreeMap<String, String>,
    documents: BTreeMap<String, CatalogEntry>,
    versions: Vec<ManagedVersion>,
    receipts: BTreeMap<String, Receipt>,
    content_bytes: u64,
}

fn hash_id(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

impl State {
    fn validate(&self) -> Result<(), Error> {
        if !hash_id(&self.server_id)
            || self.documents.len() > MAX_VERSIONS
            || self.versions.len() > MAX_VERSIONS
            || self.content_bytes > MAX_STORED_CONTENT
            || self.receipts.len() != self.versions.len()
        {
            return Err(Error::Integrity);
        }
        let mut seen = BTreeSet::new();
        let mut heads = BTreeMap::new();
        let mut paths = BTreeSet::new();
        for version in &self.versions {
            let reference = &version.version.reference;
            if !self.documents.contains_key(&reference.document)
                || reference.version == 0
                || !hash_id(&version.version.sha256)
                || !label(&version.author)
                || version.comment.len() > 1024
                || !crate::valid_id(&version.request_id)
                || version.version.dependencies.len() > MAX_DEPENDENCIES
                || version.version.parent != heads.get(&reference.document).copied()
                || reference.version != heads.get(&reference.document).copied().unwrap_or(0u64).checked_add(1).ok_or(Error::Integrity)?
            {
                return Err(Error::Integrity);
            }
            let mut dependencies = BTreeSet::new();
            for dependency in &version.version.dependencies {
                if dependency.document == reference.document || !seen.contains(dependency) || !dependencies.insert(dependency) {
                    return Err(Error::Integrity);
                }
            }
            if !seen.insert(reference.clone()) {
                return Err(Error::Integrity);
            }
            heads.insert(reference.document.clone(), reference.version);
            let receipt = self.receipts.get(&version.request_id).ok_or(Error::Integrity)?;
            if receipt.version != *version || receipt.owner != version.author || !hash_id(&receipt.fingerprint) {
                return Err(Error::Integrity);
            }
        }
        let mut locked = 0;
        for (id, entry) in &self.documents {
            if !crate::valid_id(id)
                || !id.starts_with("m-")
                || entry.document != *id
                || entry.head != heads.get(id).copied()
                || !paths.insert(entry.logical_path.to_lowercase())
            {
                return Err(Error::Integrity);
            }
            validate_logical_path(&entry.logical_path)?;
            let head = self
                .versions
                .iter()
                .find(|version| version.version.reference.document == *id && Some(version.version.reference.version) == entry.head);
            if entry.head_checked_in_unix_ms.is_some_and(|time| head.is_none_or(|version| version.checked_in_unix_ms != time))
                || entry.head_author.as_ref().is_some_and(|author| head.is_none_or(|version| version.author != *author))
            {
                return Err(Error::Integrity);
            }
            if let Some(checkout) = &entry.checkout {
                locked += 1;
                if !label(&checkout.owner)
                    || !label(&checkout.workspace)
                    || !label(&checkout.machine)
                    || checkout.base_version != entry.head
                    || !self.checkout_tokens.get(id).is_some_and(|token| hash_id(token))
                {
                    return Err(Error::Integrity);
                }
            }
        }
        if locked != self.checkout_tokens.len() {
            return Err(Error::Integrity);
        }
        Ok(())
    }
}

pub(crate) struct ManagedStore {
    root: PathBuf,
    blobs: FsBlobStore,
    generation: u64,
    state: State,
    credentials: Vec<PrincipalCredential>,
}

fn now() -> Result<u64, Error> {
    u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| Error::Invalid("clock before epoch"))?.as_millis())
        .map_err(|_| Error::Invalid("clock out of range"))
}

fn label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

/// Portable relative catalog paths; these are never joined onto server disk paths.
pub fn validate_logical_path(path: &str) -> Result<(), Error> {
    if path.is_empty() || path.len() > 240 || path.contains('\\') || path.contains(':') || path.chars().any(char::is_control) {
        return Err(Error::Invalid("logical path"));
    }
    for component in path.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.ends_with([' ', '.'])
            || component.contains(['<', '>', '"', '|', '?', '*'])
        {
            return Err(Error::Invalid("logical path component"));
        }
        let stem = component.split('.').next().unwrap_or_default().to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.ends_with(['1', '2', '3', '4', '5', '6', '7', '8', '9']))
        {
            return Err(Error::Invalid("reserved logical path"));
        }
    }
    Ok(())
}

impl ManagedStore {
    pub(crate) fn open(store_root: &Path) -> Result<Self, Error> {
        let root = store_root.join("managed");
        crate::reject_symlink_ancestors(&root)?;
        crate::reject_symlink_ancestors(&root.join("generations"))?;
        let blobs = FsBlobStore::open(&root)?;
        fs::create_dir_all(root.join("generations"))?;
        let mut generation = 0;
        for entry in fs::read_dir(root.join("generations"))? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && let Some(n) = entry.file_name().to_str().and_then(|n| n.parse::<u64>().ok())
            {
                generation = generation.max(n);
            }
        }
        if generation > MAX_GENERATIONS {
            return Err(Error::Integrity);
        }
        let state: State = if generation == 0 {
            State { server_id: crate::AuthToken::generate()?, ..State::default() }
        } else {
            serde_json::from_slice(&crate::client::read_file(&root.join("generations").join(generation.to_string()).join("state.json"), MAX_STATE)?)?
        };
        state.validate()?;
        let credential_path = store_root.join("managed-principals.json");
        let credentials: Vec<PrincipalCredential> =
            if credential_path.exists() { serde_json::from_slice(&crate::client::read_file(&credential_path, 64 * 1024)?)? } else { Vec::new() };
        let mut hashes = BTreeSet::new();
        let mut principals = BTreeSet::new();
        if credentials.len() > 64 {
            return Err(Error::Invalid("principal count"));
        }
        for item in &credentials {
            if !label(&item.principal.id)
                || item.principal.id == "owner"
                || item.token_sha256.len() != 64
                || !item.token_sha256.bytes().all(|b| b.is_ascii_hexdigit())
                || !hashes.insert(&item.token_sha256)
                || !principals.insert(&item.principal.id)
            {
                return Err(Error::Invalid("principal configuration"));
            }
        }
        let mut store = Self { root, blobs, generation, state, credentials };
        if generation == 0 {
            store.publish(store.state.clone())?;
        }
        Ok(store)
    }

    pub(crate) fn authenticate(&self, token: &str) -> Option<Principal> {
        use subtle::ConstantTimeEq;
        if token.len() != 64 {
            return None;
        }
        let hash = digest(token.as_bytes());
        self.credentials.iter().find(|item| bool::from(hash.as_bytes().ct_eq(item.token_sha256.as_bytes()))).map(|item| item.principal.clone())
    }

    fn publish(&mut self, state: State) -> Result<(), Error> {
        let next = self.generation.checked_add(1).filter(|n| *n <= MAX_GENERATIONS).ok_or(Error::Invalid("metadata generation quota"))?;
        let bytes = serde_json::to_vec(&state)?;
        if bytes.len() > MAX_STATE {
            return Err(Error::Invalid("metadata quota"));
        }
        let mut total = bytes.len() as u64;
        for entry in fs::read_dir(self.root.join("generations"))? {
            let entry = entry?;
            if entry.file_type()?.is_dir() && entry.file_name().to_str().is_some_and(|n| n.parse::<u64>().is_ok()) {
                total = total.checked_add(fs::metadata(entry.path().join("state.json"))?.len()).ok_or(Error::Invalid("metadata storage quota"))?;
                if total > MAX_METADATA_STORAGE {
                    return Err(Error::Invalid("metadata storage quota"));
                }
            }
        }
        let staging = self.root.join("generations/pending");
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir(&staging)?;
        crate::write_synced(&staging.join("state.json"), &bytes)?;
        // Sync files before publishing a complete immutable metadata generation.
        fs::rename(&staging, self.root.join("generations").join(next.to_string()))?;
        self.state = state;
        self.generation = next;
        Ok(())
    }

    pub(crate) fn contains(&self, document: &str) -> bool {
        self.state.documents.contains_key(document)
    }

    pub(crate) fn catalog(&self) -> ManagedCatalog {
        ManagedCatalog { server_id: self.state.server_id.clone(), entries: self.state.documents.values().cloned().collect() }
    }

    pub(crate) fn versions(&self) -> Vec<Version> {
        self.state.versions.iter().map(|v| v.version.clone()).collect()
    }

    pub(crate) fn history(&self, document: &str) -> Result<Vec<ManagedVersion>, Error> {
        if !self.contains(document) {
            return Err(Error::NotFound);
        }
        Ok(self.state.versions.iter().filter(|v| v.version.reference.document == document).cloned().collect())
    }

    pub(crate) fn create(&mut self, principal: &Principal, logical_path: String) -> Result<CatalogEntry, Error> {
        Self::write_permission(principal)?;
        validate_logical_path(&logical_path)?;
        if self.state.documents.len() >= MAX_VERSIONS {
            return Err(Error::Invalid("document quota"));
        }
        if let Some(entry) = self.state.documents.values().find(|e| e.logical_path.to_lowercase() == logical_path.to_lowercase()) {
            return if entry.logical_path == logical_path { Ok(entry.clone()) } else { Err(Error::Conflict) };
        }
        let document = format!("m-{}", crate::AuthToken::generate()?.get(..32).ok_or(Error::Integrity)?);
        if self.contains(&document) {
            return Err(Error::Conflict);
        }
        let entry =
            CatalogEntry { document: document.clone(), logical_path, head: None, head_checked_in_unix_ms: None, head_author: None, checkout: None };
        let mut state = self.state.clone();
        state.documents.insert(document, entry.clone());
        self.publish(state)?;
        Ok(entry)
    }

    fn write_permission(principal: &Principal) -> Result<(), Error> {
        if principal.role == Role::Viewer { Err(Error::Forbidden) } else { Ok(()) }
    }

    pub(crate) fn checkout(&mut self, principal: &Principal, document: &str, workspace: String, machine: String) -> Result<CheckoutGrant, Error> {
        Self::write_permission(principal)?;
        if !label(&workspace) || !label(&machine) {
            return Err(Error::Invalid("workspace or machine"));
        }
        let entry = self.state.documents.get(document).ok_or(Error::NotFound)?;
        if let Some(checkout) = &entry.checkout {
            if checkout.owner == principal.id && checkout.workspace == workspace && checkout.machine == machine {
                return Ok(CheckoutGrant { entry: entry.clone(), token: self.state.checkout_tokens.get(document).ok_or(Error::Integrity)?.clone() });
            }
            return Err(Error::Conflict);
        }
        let mut entry = entry.clone();
        entry.checkout = Some(Checkout { owner: principal.id.clone(), workspace, machine, checked_out_unix_ms: now()?, base_version: entry.head });
        let mut state = self.state.clone();
        let token = crate::AuthToken::generate()?;
        state.checkout_tokens.insert(document.into(), token.clone());
        state.documents.insert(document.into(), entry.clone());
        self.publish(state)?;
        Ok(CheckoutGrant { entry, token })
    }

    fn owns(&self, document: &str, checkout: &Checkout, principal: &Principal, workspace: &str, machine: &str, token: &str) -> bool {
        use subtle::ConstantTimeEq;
        checkout.owner == principal.id
            && checkout.workspace == workspace
            && checkout.machine == machine
            && self.state.checkout_tokens.get(document).is_some_and(|expected| bool::from(expected.as_bytes().ct_eq(token.as_bytes())))
    }

    pub(crate) fn cancel(
        &mut self,
        principal: &Principal,
        document: &str,
        workspace: &str,
        machine: &str,
        token: &str,
    ) -> Result<CatalogEntry, Error> {
        Self::write_permission(principal)?;
        let mut entry = self.state.documents.get(document).ok_or(Error::NotFound)?.clone();
        if !entry.checkout.as_ref().is_some_and(|c| self.owns(document, c, principal, workspace, machine, token)) {
            return Err(Error::Forbidden);
        }
        entry.checkout = None;
        let mut state = self.state.clone();
        state.checkout_tokens.remove(document);
        state.documents.insert(document.into(), entry.clone());
        self.publish(state)?;
        Ok(entry)
    }

    pub(crate) fn download(&self, reference: &VersionRef) -> Result<(Version, Vec<u8>), Error> {
        let version = &self.state.versions.iter().find(|v| v.version.reference == *reference).ok_or(Error::NotFound)?.version;
        let content = self.blobs.get_verified(&version.sha256)?;
        Ok((version.clone(), content))
    }

    pub(crate) fn check_in(&mut self, principal: &Principal, input: ManagedCheckIn) -> Result<ManagedVersion, Error> {
        Self::write_permission(principal)?;
        if !crate::valid_id(&input.request_id)
            || input.comment.len() > 1024
            || input.comment.chars().any(|c| c.is_control() && c != '\n')
            || input.content.len() > MAX_CONTENT
            || input.dependencies.len() > MAX_DEPENDENCIES
        {
            return Err(Error::Invalid("check-in limits"));
        }
        // A completed retry needs the same authenticated principal and exact payload,
        // not a checkout secret that the client may have lost after the acknowledgement.
        let mut receipt_input = input.clone();
        receipt_input.checkout_token.clear();
        let fingerprint = digest(&serde_json::to_vec(&receipt_input)?);
        if let Some(receipt) = self.state.receipts.get(&input.request_id) {
            return if receipt.owner == principal.id && receipt.fingerprint == fingerprint {
                Ok(receipt.version.clone())
            } else {
                Err(Error::Conflict)
            };
        }
        let mut entry = self.state.documents.get(&input.document).ok_or(Error::NotFound)?.clone();
        let checkout = entry.checkout.as_ref().ok_or(Error::Forbidden)?;
        if !self.owns(&input.document, checkout, principal, &input.workspace, &input.machine, &input.checkout_token) {
            return Err(Error::Forbidden);
        }
        if input.parent != entry.head || input.parent != checkout.base_version {
            return Err(Error::Conflict);
        }
        if self.state.versions.len() >= MAX_VERSIONS
            || self.state.content_bytes.checked_add(input.content.len() as u64).is_none_or(|bytes| bytes > MAX_STORED_CONTENT)
        {
            return Err(Error::Invalid("managed storage quota"));
        }
        let mut unique = BTreeSet::new();
        for dependency in &input.dependencies {
            if dependency.document == input.document || !unique.insert(dependency) {
                return Err(Error::Invalid("self or duplicate dependency"));
            }
            self.download(dependency)?;
        }
        let number = entry.head.unwrap_or(0).checked_add(1).ok_or(Error::Invalid("version exhausted"))?;
        let version = ManagedVersion {
            version: Version {
                reference: VersionRef { document: input.document.clone(), version: number },
                parent: input.parent,
                sha256: digest(&input.content),
                dependencies: input.dependencies,
            },
            author: principal.id.clone(),
            checked_in_unix_ms: now()?,
            comment: input.comment,
            request_id: input.request_id.clone(),
        };
        self.blobs.put_verified_if_absent(&version.version.sha256, &input.content)?;
        // The receipt, head, immutable version metadata and unlock share one publication.
        // A failed metadata commit leaves the old checkout intact and only an orphan blob.
        entry.head = Some(number);
        entry.head_checked_in_unix_ms = Some(version.checked_in_unix_ms);
        entry.head_author = Some(version.author.clone());
        entry.checkout = None;
        let mut state = self.state.clone();
        state.content_bytes += input.content.len() as u64;
        state.checkout_tokens.remove(&input.document);
        state.documents.insert(input.document, entry);
        state.versions.push(version.clone());
        state.receipts.insert(input.request_id, Receipt { owner: principal.id.clone(), fingerprint, version: version.clone() });
        self.publish(state)?;
        Ok(version)
    }

    pub(crate) fn where_used(&self, reference: &VersionRef) -> Vec<VersionRef> {
        self.state.versions.iter().filter(|v| v.version.dependencies.contains(reference)).map(|v| v.version.reference.clone()).collect()
    }
}
