//! Native worker login/resume foundation. No constructor performs network or
//! credential-store operations. Profiles contain public identity and trust only.
#![cfg(not(target_arch = "wasm32"))]
use crate::pdm_credentials::{BearerSecret, CredentialContext, CredentialKey, CredentialStore, Purpose, normalized_origin};
use serde::{Deserialize, Serialize};
use solidcraft_pdm::{AuthToken, AuthenticatedIdentity, client::Client};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginError {
    InvalidProfile,
    AuthenticationFailed,
    CredentialUnavailable,
    CredentialMissing,
    IdentityMismatch,
    EnrollmentFailed,
}
impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidProfile => "Invalid PDM connection profile",
            Self::AuthenticationFailed => "PDM authentication failed",
            Self::CredentialUnavailable => "PDM credential storage unavailable",
            Self::CredentialMissing => "PDM login required",
            Self::IdentityMismatch => "PDM authenticated identity mismatch",
            Self::EnrollmentFailed => "PDM local enrollment failed; preserve existing credentials and store",
        })
    }
}
impl std::error::Error for LoginError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "certificate_der", rename_all = "snake_case", deny_unknown_fields)]
enum TrustData {
    Platform,
    Certificate(Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct Trust(TrustData);
impl Trust {
    pub fn platform() -> Self {
        Self(TrustData::Platform)
    }
    pub fn certificate_der(der: &[u8]) -> Result<Self, LoginError> {
        if der.is_empty() || der.len() > 64 * 1024 {
            return Err(LoginError::InvalidProfile);
        }
        let mut roots = rustls::RootCertStore::empty();
        roots.add(rustls::pki_types::CertificateDer::from(der.to_vec())).map_err(|_| LoginError::InvalidProfile)?;
        Ok(Self(TrustData::Certificate(der.to_vec())))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetData {
    origin: String,
    tunnel: Option<String>,
    server_id: String,
    trust: TrustData,
}

#[derive(Clone, Debug)]
pub struct LoginTarget(TargetData);
impl LoginTarget {
    pub fn direct(origin: &str, server_id: &str, trust: Trust) -> Result<Self, LoginError> {
        Self::validated(origin, None, server_id, trust)
    }
    pub fn relay(origin: &str, tunnel: &str, server_id: &str, trust: Trust) -> Result<Self, LoginError> {
        Self::validated(origin, Some(tunnel), server_id, trust)
    }
    fn validated(origin: &str, tunnel: Option<&str>, server_id: &str, trust: Trust) -> Result<Self, LoginError> {
        let origin = normalized_origin(origin).map_err(|_| LoginError::InvalidProfile)?;
        CredentialKey::new(&origin, server_id, "validation", Purpose::User).map_err(|_| LoginError::InvalidProfile)?;
        if tunnel.is_some_and(|tunnel| {
            tunnel.is_empty() || tunnel.len() > 64 || !tunnel.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        }) {
            return Err(LoginError::InvalidProfile);
        }
        let trust = match trust.0 {
            TrustData::Platform => Trust::platform(),
            TrustData::Certificate(der) => Trust::certificate_der(&der)?,
        };
        Ok(Self(TargetData { origin, tunnel: tunnel.map(str::to_owned), server_id: server_id.into(), trust: trust.0 }))
    }
    fn client(&self, secret: &BearerSecret) -> Result<Client, LoginError> {
        let token = AuthToken::parse(secret.expose().into()).map_err(|_| LoginError::AuthenticationFailed)?;
        let target = &self.0;
        let client = match (&target.tunnel, &target.trust) {
            (None, TrustData::Platform) => Client::https(&target.origin, token),
            (None, TrustData::Certificate(der)) => Client::https_with_ca(&target.origin, token, der),
            (Some(tunnel), TrustData::Platform) => Client::https_relay(&target.origin, tunnel, token),
            (Some(tunnel), TrustData::Certificate(der)) => Client::https_relay_with_ca(&target.origin, tunnel, token, der),
        }
        .map_err(|_| LoginError::InvalidProfile)?;
        client.bind_server(&target.server_id).map_err(|_| LoginError::InvalidProfile)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileData {
    target: TargetData,
    principal_id: String,
}

/// Serializable, credential-free saved profile. Deserialization revalidates every
/// field. An identity label in a saved profile never establishes authentication.
/// Prefer `from_json` for bounded input; direct Deserialize callers must bound
/// input before invoking serde (including certificate array allocation).
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct Profile(ProfileData);
impl<'de> Deserialize<'de> for Profile {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let data = ProfileData::deserialize(deserializer)?;
        Self::from_data(data).map_err(serde::de::Error::custom)
    }
}
impl Profile {
    pub fn from_json(bytes: &[u8]) -> Result<Self, LoginError> {
        if bytes.len() > 384 * 1024 {
            return Err(LoginError::InvalidProfile);
        }
        serde_json::from_slice(bytes).map_err(|_| LoginError::InvalidProfile)
    }
    fn from_data(data: ProfileData) -> Result<Self, LoginError> {
        let target =
            LoginTarget::validated(&data.target.origin, data.target.tunnel.as_deref(), &data.target.server_id, Trust(data.target.trust.clone()))?;
        let profile = Self(ProfileData { target: target.0, principal_id: data.principal_id });
        profile.key()?;
        if profile.0.principal_id == "owner" {
            return Err(LoginError::InvalidProfile);
        }
        Ok(profile)
    }
    fn key(&self) -> Result<CredentialKey, LoginError> {
        let target = &self.0.target;
        let context = match &target.trust {
            TrustData::Platform => CredentialContext::platform(target.tunnel.as_deref()),
            TrustData::Certificate(der) => CredentialContext::certificate_der(der, target.tunnel.as_deref()),
        }
        .map_err(|_| LoginError::InvalidProfile)?;
        CredentialKey::new_scoped(&target.origin, &target.server_id, &self.0.principal_id, Purpose::User, &context)
            .map_err(|_| LoginError::InvalidProfile)
    }
    pub fn principal_id(&self) -> &str {
        &self.0.principal_id
    }
    pub fn server_id(&self) -> &str {
        &self.0.target.server_id
    }
    pub fn origin(&self) -> &str {
        &self.0.target.origin
    }
    pub fn tunnel(&self) -> Option<&str> {
        self.0.target.tunnel.as_deref()
    }
}

/// Client Debug redacts its bearer. Role is fetched fresh on every login/resume.
#[derive(Debug)]
pub struct AuthenticatedSession {
    pub profile: Profile,
    pub identity: AuthenticatedIdentity,
    pub client: Client,
}

/// Invoke on a native worker. Save the user slot only after verified server auth.
pub fn login(target: LoginTarget, secret: BearerSecret, credentials: &impl CredentialStore) -> Result<AuthenticatedSession, LoginError> {
    let client = target.client(&secret)?;
    let identity = client.authenticated_identity().map_err(|_| LoginError::AuthenticationFailed)?;
    let profile = verified_profile(target, &identity)?;
    credentials.set(&profile.key()?, &secret).map_err(|_| LoginError::CredentialUnavailable)?;
    Ok(AuthenticatedSession { profile, identity, client })
}

fn verified_profile(target: LoginTarget, identity: &AuthenticatedIdentity) -> Result<Profile, LoginError> {
    if identity.server_id != target.0.server_id {
        return Err(LoginError::IdentityMismatch);
    }
    Profile::from_data(ProfileData { target: target.0, principal_id: identity.principal.id.clone() })
}

/// Bind the expected server before the first request and reauthenticate identity.
pub fn resume(profile: Profile, credentials: &impl CredentialStore) -> Result<AuthenticatedSession, LoginError> {
    let profile = Profile::from_data(profile.0)?;
    let secret = credentials.get(&profile.key()?).map_err(|_| LoginError::CredentialUnavailable)?.ok_or(LoginError::CredentialMissing)?;
    let client = LoginTarget(profile.0.target.clone()).client(&secret)?;
    let identity = client.authenticated_identity().map_err(|_| LoginError::AuthenticationFailed)?;
    if identity.server_id != profile.server_id() || identity.principal.id != profile.principal_id() {
        return Err(LoginError::IdentityMismatch);
    }
    Ok(AuthenticatedSession { profile, identity, client })
}

pub fn forget(profile: &Profile, credentials: &impl CredentialStore) -> Result<(), LoginError> {
    credentials.delete(&profile.key()?).map_err(|_| LoginError::CredentialUnavailable)
}

/// Native installer worker boundary. Capture the first administrator into its
/// user slot before publishing the registry. On publication failure retain the
/// captured credential for recovery; never reset the store or delete the slot.
/// This does not start a service or perform network login.
pub fn enroll_local(
    root: &std::path::Path,
    administrator: &str,
    origin: &str,
    trust: Trust,
    credentials: &impl CredentialStore,
) -> Result<(solidcraft_pdm::bootstrap::EnrollmentReceipt, Profile), LoginError> {
    if !root.is_absolute() || root.components().any(|part| matches!(part, std::path::Component::ParentDir)) {
        return Err(LoginError::InvalidProfile);
    }
    // Validate endpoint/trust before creating any store or credential entry.
    let validated = LoginTarget::direct(origin, &"0".repeat(64), trust)?;
    let mut profile = None;
    let mut credential_failure = false;
    let receipt = solidcraft_pdm::bootstrap::initialize_to(root, administrator, |receipt, bearer| {
        let target = LoginTarget::direct(&validated.0.origin, &receipt.server_id, Trust(validated.0.trust.clone()))
            .map_err(|_| solidcraft_pdm::Error::Invalid("native enrollment profile"))?;
        let candidate = Profile::from_data(ProfileData { target: target.0, principal_id: receipt.administrator.clone() })
            .map_err(|_| solidcraft_pdm::Error::Invalid("native enrollment identity"))?;
        let secret = BearerSecret::parse(bearer.into()).map_err(|_| solidcraft_pdm::Error::Invalid("native enrollment credential"))?;
        let key = candidate.key().map_err(|_| solidcraft_pdm::Error::Invalid("native enrollment key"))?;
        if credentials.set(&key, &secret).is_err() {
            credential_failure = true;
            return Err(solidcraft_pdm::Error::Invalid("native credential storage unavailable"));
        }
        profile = Some(candidate);
        Ok(())
    })
    .map_err(|_| if credential_failure { LoginError::CredentialUnavailable } else { LoginError::EnrollmentFailed })?;
    Ok((receipt, profile.ok_or(LoginError::EnrollmentFailed)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdm_credentials::CredentialError;
    use solidcraft_pdm::managed::{Principal, Role};
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeMap,
    };
    #[derive(Default)]
    struct Memory {
        values: RefCell<BTreeMap<CredentialKey, String>>,
        writes: Cell<usize>,
        fail_writes: Cell<bool>,
    }
    impl CredentialStore for Memory {
        fn get(&self, key: &CredentialKey) -> Result<Option<BearerSecret>, CredentialError> {
            self.values.borrow().get(key).cloned().map(BearerSecret::parse).transpose()
        }
        fn set(&self, key: &CredentialKey, secret: &BearerSecret) -> Result<(), CredentialError> {
            if self.fail_writes.get() {
                return Err(CredentialError::Unavailable);
            }
            self.values.borrow_mut().insert(key.clone(), secret.expose().into());
            self.writes.set(self.writes.get() + 1);
            Ok(())
        }
        fn delete(&self, key: &CredentialKey) -> Result<(), CredentialError> {
            self.values.borrow_mut().remove(key);
            Ok(())
        }
    }
    fn profile() -> Result<Profile, LoginError> {
        let target = LoginTarget::direct("https://example.com", &"a".repeat(64), Trust::platform())?;
        verified_profile(
            target,
            &AuthenticatedIdentity { server_id: "a".repeat(64), principal: Principal { id: "actual-admin".into(), role: Role::Administrator } },
        )
    }
    #[test]
    fn absent_corrupt_and_other_purpose_fail_before_network() -> Result<(), Box<dyn std::error::Error>> {
        let credentials = Memory::default();
        assert!(matches!(resume(profile()?, &credentials), Err(LoginError::CredentialMissing)));
        let connector = CredentialKey::new("https://example.com", &"a".repeat(64), "actual-admin", Purpose::Connector)?;
        credentials.set(&connector, &BearerSecret::parse("b".repeat(64))?)?;
        assert!(matches!(resume(profile()?, &credentials), Err(LoginError::CredentialMissing)));
        credentials.values.borrow_mut().insert(profile()?.key()?, "corrupt-sensitive-value".into());
        assert!(matches!(resume(profile()?, &credentials), Err(LoginError::CredentialUnavailable)));
        forget(&profile()?, &credentials)?;
        assert!(credentials.get(&connector)?.is_some());
        assert!(matches!(resume(profile()?, &credentials), Err(LoginError::CredentialMissing)));
        Ok(())
    }
    #[test]
    fn profiles_validate_on_deserialization_and_never_include_bearers() -> Result<(), Box<dyn std::error::Error>> {
        let serialized = serde_json::to_string(&profile()?)?;
        let restored: Profile = serde_json::from_str(&serialized)?;
        assert_eq!(restored.principal_id(), "actual-admin");
        assert_eq!(restored.origin(), "https://example.com");
        for modified in [
            serialized.replace("https://example.com", "http://example.com"),
            serialized.replace("actual-admin", "owner"),
            serialized.replace("actual-admin", ""),
        ] {
            assert!(serde_json::from_str::<Profile>(&modified).is_err());
        }
        assert!(Trust::certificate_der(&[]).is_err());
        assert!(Trust::certificate_der(&[1, 2, 3]).is_err());
        assert!(Trust::certificate_der(&vec![0; 65537]).is_err());
        assert!(LoginTarget::relay("https://example.com", "../bad", &"a".repeat(64), Trust::platform()).is_err());
        Ok(())
    }
    #[test]
    fn authenticated_server_mismatch_cannot_form_a_profile() -> Result<(), LoginError> {
        let target = LoginTarget::direct("https://example.com", &"a".repeat(64), Trust::platform())?;
        let identity = AuthenticatedIdentity { server_id: "b".repeat(64), principal: Principal { id: "admin".into(), role: Role::Viewer } };
        assert!(matches!(verified_profile(target, &identity), Err(LoginError::IdentityMismatch)));
        assert_eq!(LoginError::AuthenticationFailed.to_string(), "PDM authentication failed");
        Ok(())
    }

    #[test]
    fn imported_trust_and_route_changes_cannot_retrieve_existing_bearer() -> Result<(), Box<dyn std::error::Error>> {
        let credentials = Memory::default();
        let ca = include_bytes!("../../solidcraft-pdm/tests/fixtures/pdm-https/ca.der");
        let other_ca = include_bytes!("../../solidcraft-pdm/tests/fixtures/pdm-https/server.der");
        let original = Profile::from_data(ProfileData {
            target: LoginTarget::direct("https://EXAMPLE.COM:443/", &"a".repeat(64), Trust::certificate_der(ca)?)?.0,
            principal_id: "Alice".into(),
        })?;
        let key = original.key()?;
        credentials.set(&key, &BearerSecret::parse("c".repeat(64))?)?;
        let canonical = Profile::from_data(ProfileData {
            target: LoginTarget::direct("https://example.com", &"a".repeat(64), Trust::certificate_der(ca)?)?.0,
            principal_id: "Alice".into(),
        })?;
        assert_eq!(canonical.key()?, key);
        let serialized = serde_json::to_vec(&original)?;
        let mut changes = Vec::new();
        let mut changed_ca: ProfileData = serde_json::from_slice(&serialized)?;
        changed_ca.target.trust = Trust::certificate_der(other_ca)?.0;
        changes.push(changed_ca);
        let mut changed_platform: ProfileData = serde_json::from_slice(&serialized)?;
        changed_platform.target.trust = TrustData::Platform;
        changes.push(changed_platform);
        let mut changed_route: ProfileData = serde_json::from_slice(&serialized)?;
        changed_route.target.tunnel = Some("tunnel-a".into());
        changes.push(changed_route);
        for changed in changes {
            let imported = Profile::from_json(&serde_json::to_vec(&changed)?)?;
            assert_ne!(imported.key()?, key);
            assert!(matches!(resume(imported, &credentials), Err(LoginError::CredentialMissing)));
        }
        let relay_a = Profile::from_data(ProfileData {
            target: LoginTarget::relay("https://example.com", "tunnel-a", &"a".repeat(64), Trust::certificate_der(ca)?)?.0,
            principal_id: "Alice".into(),
        })?;
        credentials.set(&relay_a.key()?, &BearerSecret::parse("d".repeat(64))?)?;
        let mut relay_b: ProfileData = serde_json::from_slice(&serde_json::to_vec(&relay_a)?)?;
        relay_b.target.tunnel = Some("tunnel-b".into());
        assert!(matches!(resume(Profile::from_json(&serde_json::to_vec(&relay_b)?)?, &credentials), Err(LoginError::CredentialMissing)));
        assert!(credentials.get(&key)?.is_some());
        assert!(credentials.get(&relay_a.key()?)?.is_some());
        assert!(matches!(Profile::from_json(&vec![b' '; 384 * 1024 + 1]), Err(LoginError::InvalidProfile)));
        Ok(())
    }

    struct TestService {
        server: Option<solidcraft_pdm::http_server::HttpsServer>,
        root: std::path::PathBuf,
        server_id: String,
    }
    impl Drop for TestService {
        fn drop(&mut self) {
            if let Some(server) = self.server.take() {
                let _ = server.shutdown();
            }
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    impl TestService {
        fn start(bearer: &str) -> Result<Self, Box<dyn std::error::Error>> {
            use sha2::{Digest, Sha256};
            use solidcraft_pdm::{
                Request, Response, Store,
                http_server::{HttpsConfig, HttpsServer},
                managed::PrincipalCredential,
            };
            let root = std::env::temp_dir().join(format!("solidcraft-native-login-{}", AuthToken::generate()?));
            // Construct the guard before I/O so failed setup also removes its root.
            let mut service = Self { server: None, root, server_id: String::new() };
            std::fs::create_dir(&service.root)?;
            let credentials = [PrincipalCredential {
                principal: Principal { id: "Alice".into(), role: Role::Contributor },
                token_sha256: format!("{:x}", Sha256::digest(bearer.as_bytes())),
            }];
            std::fs::write(service.root.join("managed-principals.json"), serde_json::to_vec(&credentials)?)?;
            let mut store = Store::open(&service.root)?;
            service.server_id = match store.execute(Request::ManagedCatalog) {
                Response::ManagedCatalog { catalog } => catalog.server_id,
                _ => return Err(LoginError::InvalidProfile.into()),
            };
            let tls = HttpsConfig::from_der(
                "127.0.0.1:0".parse()?,
                vec![include_bytes!("../../solidcraft-pdm/tests/fixtures/pdm-https/server.der").to_vec()],
                include_bytes!("../../solidcraft-pdm/tests/fixtures/pdm-https/server-key.der").to_vec(),
            )?;
            service.server = Some(HttpsServer::start(store, tls)?);
            Ok(service)
        }
        fn target(&self) -> Result<LoginTarget, LoginError> {
            let server = self.server.as_ref().ok_or(LoginError::InvalidProfile)?;
            LoginTarget::direct(
                &format!("https://{}", server.address()),
                &self.server_id,
                Trust::certificate_der(include_bytes!("../../solidcraft-pdm/tests/fixtures/pdm-https/ca.der"))?,
            )
        }
    }

    #[test]
    fn actual_https_login_resume_and_forget_are_identity_fenced() -> Result<(), Box<dyn std::error::Error>> {
        let bearer = AuthToken::generate()?;
        let first = TestService::start(&bearer)?;
        let credentials = Memory::default();
        let wrong = AuthToken::generate()?;
        assert!(matches!(login(first.target()?, BearerSecret::parse(wrong)?, &credentials), Err(LoginError::AuthenticationFailed)));
        assert_eq!(credentials.writes.get(), 0);
        let session = login(first.target()?, BearerSecret::parse(bearer.clone())?, &credentials)?;
        assert_eq!(credentials.writes.get(), 1);
        assert_eq!(session.profile.principal_id(), "Alice");
        assert_eq!(session.identity.principal.role, Role::Contributor);
        let serialized = serde_json::to_string(&session.profile)?;
        assert!(!serialized.contains(&bearer));
        assert!(!format!("{session:?}").contains(&bearer));
        let resumed = resume(serde_json::from_str(&serialized)?, &credentials)?;
        assert_eq!(resumed.identity.principal.id, "Alice");
        assert_eq!(resumed.identity.principal.role, Role::Contributor);
        assert_eq!(resumed.identity.server_id, first.server_id);
        assert_eq!(credentials.writes.get(), 1);

        // Same CA and configured bearer do not authorize substituting a different
        // store: bind the original ID before the first RPC to the second endpoint.
        let second = TestService::start(&bearer)?;
        assert_ne!(first.server_id, second.server_id);
        let mut substituted = second.target()?;
        substituted.0.server_id = first.server_id.clone();
        assert!(matches!(login(substituted, BearerSecret::parse(bearer.clone())?, &credentials), Err(LoginError::AuthenticationFailed)));
        assert_eq!(credentials.writes.get(), 1);
        let connector_key = CredentialKey::new(session.profile.origin(), session.profile.server_id(), "Alice", Purpose::Connector)?;
        credentials.set(&connector_key, &BearerSecret::parse(AuthToken::generate()?)?)?;
        forget(&session.profile, &credentials)?;
        assert!(credentials.get(&connector_key)?.is_some());
        assert!(matches!(resume(serde_json::from_str(&serialized)?, &credentials), Err(LoginError::CredentialMissing)));
        Ok(())
    }

    #[test]
    fn enrollment_captures_admin_before_publication_and_retries_sink_failure() -> Result<(), Box<dyn std::error::Error>> {
        use solidcraft_pdm::{
            Store,
            http_server::{HttpsConfig, HttpsServer},
        };
        let mut service = TestService {
            server: None,
            root: std::env::temp_dir().join(format!("solidcraft-native-enroll-{}", AuthToken::generate()?)),
            server_id: String::new(),
        };
        let credentials = Memory::default();
        credentials.fail_writes.set(true);
        assert!(matches!(
            enroll_local(&service.root, "admin", "https://127.0.0.1", Trust::platform(), &credentials),
            Err(LoginError::CredentialUnavailable)
        ));
        assert!(!service.root.join("managed-principals.json").try_exists()?);
        assert!(credentials.values.borrow().is_empty());
        credentials.fail_writes.set(false);
        let (receipt, profile) = enroll_local(&service.root, "admin", "https://127.0.0.1", Trust::platform(), &credentials)?;
        assert_eq!(receipt.server_id, profile.server_id());
        assert_eq!(receipt.administrator, profile.principal_id());
        assert_eq!(credentials.writes.get(), 1);
        let captured = credentials.get(&profile.key()?)?.ok_or(LoginError::CredentialMissing)?;
        assert!(!serde_json::to_string(&profile)?.contains(captured.expose()));
        assert!(matches!(
            enroll_local(&service.root, "other-admin", "https://127.0.0.1", Trust::platform(), &credentials),
            Err(LoginError::EnrollmentFailed)
        ));
        assert_eq!(credentials.writes.get(), 1);
        let tls = HttpsConfig::from_der(
            "127.0.0.1:0".parse()?,
            vec![include_bytes!("../../solidcraft-pdm/tests/fixtures/pdm-https/server.der").to_vec()],
            include_bytes!("../../solidcraft-pdm/tests/fixtures/pdm-https/server-key.der").to_vec(),
        )?;
        let server = HttpsServer::start(Store::open(&service.root)?, tls)?;
        let endpoint = format!("https://{}", server.address());
        service.server = Some(server);
        let client = Client::https_with_ca(
            &endpoint,
            AuthToken::parse(captured.expose().into())?,
            include_bytes!("../../solidcraft-pdm/tests/fixtures/pdm-https/ca.der"),
        )?
        .bind_server(profile.server_id())?;
        let identity = client.authenticated_identity()?;
        assert_eq!(identity.principal.id, "admin");
        assert_eq!(identity.principal.role, Role::Administrator);
        assert_eq!(identity.server_id, receipt.server_id);
        Ok(())
    }
}
