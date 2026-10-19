//! Disposable packaging rehearsal; no real secrets, keyring or public listener.
use solidcraft_pdm::{
    AuthToken, Store, bootstrap,
    client::Client,
    deployment::{DeploymentConfig, start_loopback_rehearsal},
    managed::ManagedCheckIn,
};
use std::{fs, path::PathBuf};
use std::{
    io::{BufRead, Read, Write},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
const CA: &[u8] = include_bytes!("fixtures/pdm-https/ca.der");
const CERT: &[u8] = include_bytes!("fixtures/pdm-https/server.der");
const KEY: &[u8] = include_bytes!("fixtures/pdm-https/server-key.der");
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("pdm-deployment-{}", AuthToken::generate().unwrap()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn config(root: &Temp) -> DeploymentConfig {
    fs::write(root.0.join("server.der"), CERT).unwrap();
    fs::write(root.0.join("key.der"), KEY).unwrap();
    DeploymentConfig {
        store: root.0.join("store"),
        bind: "127.0.0.1:0".parse().unwrap(),
        certificate_chain: vec![root.0.join("server.der")],
        private_key: root.0.join("key.der"),
    }
}
fn enroll(config: &DeploymentConfig) -> (String, String) {
    let mut bearer = String::new();
    let receipt = bootstrap::initialize_to(&config.store, "admin", |_, secret| {
        bearer = secret.into();
        Ok(())
    })
    .unwrap();
    (receipt.server_id, bearer)
}
#[test]
fn enrolled_service_rehearsal_publishes_and_reopens_the_same_identity_and_content() {
    let root = Temp::new();
    let config = config(&root);
    let (id, bearer) = enroll(&config);
    let path = root.0.join("deployment.json");
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let server = start_loopback_rehearsal(DeploymentConfig::load(&path).unwrap()).unwrap();
    assert_eq!(server.receipt().server_id, id);
    let receipt = serde_json::to_string(server.receipt()).unwrap();
    assert!(!receipt.contains(&bearer));
    assert!(Store::open(&config.store).is_err());
    let client =
        Client::https_with_ca(&format!("https://127.0.0.1:{}", server.receipt().address.port()), AuthToken::parse(bearer.clone()).unwrap(), CA)
            .unwrap()
            .bind_server(&id)
            .unwrap();
    let entry = client.managed_create("parts/toy.solidcraft".into()).unwrap();
    let grant = client.managed_checkout(entry.document.clone(), "test-workspace".into(), "test-machine".into()).unwrap();
    let version = client
        .managed_check_in(ManagedCheckIn {
            document: entry.document.clone(),
            workspace: "test-workspace".into(),
            machine: "test-machine".into(),
            checkout_token: grant.token,
            request_id: "deployment-initial".into(),
            parent: None,
            content: b"original toy bytes".to_vec(),
            dependencies: vec![],
            comment: "Packaging rehearsal".into(),
        })
        .unwrap();
    server.shutdown().unwrap();
    let server = start_loopback_rehearsal(config).unwrap();
    assert_eq!(server.receipt().server_id, id);
    let client = Client::https_with_ca(&format!("https://127.0.0.1:{}", server.receipt().address.port()), AuthToken::parse(bearer).unwrap(), CA)
        .unwrap()
        .bind_server(&id)
        .unwrap();
    let (_, bytes) = client.download(&version.version.reference).unwrap();
    assert_eq!(bytes, b"original toy bytes");
    assert!(client.managed_catalog().unwrap().entries.iter().all(|entry| entry.checkout.is_none()));
    server.shutdown().unwrap();
}
#[test]
fn invalid_configuration_never_creates_a_store_or_accepts_embedded_secrets() {
    let root = Temp::new();
    let mut config = config(&root);
    config.bind = "0.0.0.0:443".parse().unwrap();
    assert!(start_loopback_rehearsal(config).is_err());
    assert!(!root.0.join("store").exists());
    let path = root.0.join("config.json");
    let config = config_for_invalid(&root);
    let mut value = serde_json::to_value(&config).unwrap();
    value.as_object_mut().unwrap().insert("token".into(), serde_json::json!("not-allowed"));
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(DeploymentConfig::load(&path).is_err());
    fs::write(&path, vec![b' '; 64 * 1024 + 1]).unwrap();
    assert!(DeploymentConfig::load(&path).is_err());
    assert!(DeploymentConfig::load(&root.0.join("missing.json")).is_err());
    assert!(start_loopback_rehearsal(config_for_invalid(&root)).is_err());
    assert!(!root.0.join("store").exists());
}
fn config_for_invalid(root: &Temp) -> DeploymentConfig {
    config(root)
}
#[test]
fn unconfigured_store_bad_tls_parent_paths_and_occupied_ports_fail_closed() {
    let root = Temp::new();
    let mut config = config(&root);
    fs::create_dir(&config.store).unwrap();
    assert!(start_loopback_rehearsal(config_for_invalid(&root)).is_err());
    assert!(!config.store.join("writer.lock").exists());
    config.private_key = root.0.join("missing/../key.der");
    assert!(start_loopback_rehearsal(config).is_err());
    let config = config_for_invalid(&root);
    let _ = enroll(&config);
    fs::write(&config.private_key, b"invalid PKCS8").unwrap();
    assert!(start_loopback_rehearsal(config).is_err());
    assert!(Store::open(&root.0.join("store")).is_ok());
    let mut config = config_for_invalid(&root);
    let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    config.bind = occupied.local_addr().unwrap();
    assert!(start_loopback_rehearsal(config).is_err());
    assert!(Store::open(&root.0.join("store")).is_ok());
}
#[cfg(unix)]
#[test]
fn symlink_certificate_is_rejected() {
    let root = Temp::new();
    let mut config = config(&root);
    let _ = enroll(&config);
    let link = root.0.join("linked.der");
    std::os::unix::fs::symlink(&config.certificate_chain[0], &link).unwrap();
    config.certificate_chain = vec![link];
    assert!(start_loopback_rehearsal(config).is_err());
}
#[cfg(windows)]
#[test]
fn reparse_certificate_parent_is_rejected() {
    use std::os::windows::fs::symlink_dir;
    let root = Temp::new();
    let mut config = config(&root);
    let _ = enroll(&config);
    let link = root.0.join("linked");
    // Creating symlinks needs Developer Mode or privilege on Windows. A junction
    // test belongs in the native installer gate when this host cannot create one.
    if symlink_dir(&root.0, &link).is_err() {
        return;
    }
    config.certificate_chain = vec![link.join("server.der")];
    assert!(start_loopback_rehearsal(config).is_err());
    fs::remove_dir(link).unwrap();
}

struct OwnChild(Child);
impl OwnChild {
    fn wait_bounded(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "owned rehearsal child did not stop before its deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for OwnChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn spawn_rehearsal(path: &std::path::Path) -> OwnChild {
    OwnChild(
        Command::new(env!("CARGO_BIN_EXE_solidcraft-pdm"))
            .arg("rehearse-https")
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}
#[test]
fn real_cli_reports_readiness_serves_https_and_releases_its_store_on_shutdown() {
    let root = Temp::new();
    let config = config(&root);
    let (server_id, bearer) = enroll(&config);
    let path = root.0.join("deployment.json");
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut child = spawn_rehearsal(&path);
    let stdout = child.0.stdout.take().unwrap();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        let result = std::io::BufReader::new(stdout).take(1025).read_line(&mut line).map(|_| line);
        let _ = send.send(result);
    });
    let line = match receive.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(line)) => line,
        _ => {
            drop(child);
            reader.join().unwrap();
            panic!("owned rehearsal child failed readiness");
        }
    };
    reader.join().unwrap();
    assert!(line.len() <= 1024 && line.ends_with('\n'));
    assert!(!line.contains(&bearer));
    let receipt: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(receipt.get("server_id").and_then(|v| v.as_str()), Some(server_id.as_str()));
    let address: std::net::SocketAddr = receipt.get("address").and_then(|v| v.as_str()).unwrap().parse().unwrap();
    assert!(address.ip().is_loopback());
    assert_ne!(address.port(), 0);
    let client = Client::https_with_ca(&format!("https://127.0.0.1:{}", address.port()), AuthToken::parse(bearer).unwrap(), CA)
        .unwrap()
        .bind_server(&server_id)
        .unwrap();
    assert_eq!(client.managed_catalog().unwrap().server_id, server_id);
    let entry = client.managed_create("cli/toy.solidcraft".into()).unwrap();
    assert_eq!(entry.logical_path, "cli/toy.solidcraft");
    let grant = client.managed_checkout(entry.document.clone(), "cli-workspace".into(), "cli-machine".into()).unwrap();
    let version = client
        .managed_check_in(ManagedCheckIn {
            document: entry.document,
            workspace: "cli-workspace".into(),
            machine: "cli-machine".into(),
            checkout_token: grant.token,
            request_id: "cli-publish".into(),
            parent: None,
            content: b"disposable CLI toy".to_vec(),
            dependencies: vec![],
            comment: "CLI service rehearsal".into(),
        })
        .unwrap();
    assert_eq!(client.download(&version.version.reference).unwrap().1, b"disposable CLI toy");
    child.0.stdin.as_mut().unwrap().write_all(b"shutdown\n").unwrap();
    child.0.stdin.as_mut().unwrap().flush().unwrap();
    assert!(child.wait_bounded().success());
    assert!(Store::open(&config.store).is_ok());
}
#[test]
fn real_cli_rejects_public_address_without_creating_a_store() {
    let root = Temp::new();
    let mut config = config(&root);
    config.bind = "0.0.0.0:443".parse().unwrap();
    let path = root.0.join("deployment.json");
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut child = spawn_rehearsal(&path);
    drop(child.0.stdin.take());
    assert!(!child.wait_bounded().success());
    assert!(!config.store.exists());
}
