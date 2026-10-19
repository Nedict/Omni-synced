//! All listeners and credentials in this suite are disposable loopback fixtures.
use sha2::{Digest, Sha256};
use solidcraft_pdm::{
    HTTP_RPC_PATH, PROTOCOL, Request, Response, Store,
    http_server::{HttpsConfig, HttpsServer},
    managed::ManagedCheckIn,
};
use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

const CA: &[u8] = include_bytes!("fixtures/pdm-https/ca.der");
const CERT: &[u8] = include_bytes!("fixtures/pdm-https/server.der");
const KEY: &[u8] = include_bytes!("fixtures/pdm-https/server-key.der");
const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const VIEWER: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("pdm-https-{}-{}", std::process::id(), solidcraft_pdm::AuthToken::generate().unwrap()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn config() -> HttpsConfig {
    HttpsConfig::from_der("127.0.0.1:0".parse().unwrap(), vec![CERT.to_vec()], KEY.to_vec()).unwrap()
}
fn server(root: &Temp) -> HttpsServer {
    let users: Vec<_> = [("alice", "contributor", ALICE), ("bob", "contributor", BOB), ("reader", "viewer", VIEWER)].into_iter().map(|(id,role,token)| {
        serde_json::json!({"principal":{"id":id,"role":role},"token_sha256":format!("{:x}",Sha256::digest(token.as_bytes()))})
    }).collect();
    fs::write(root.0.join("managed-principals.json"), serde_json::to_vec(&users).unwrap()).unwrap();
    HttpsServer::start(Store::open(&root.0).unwrap(), config()).unwrap()
}
fn tls(address: SocketAddr, trusted: bool, name: &'static str) -> rustls::StreamOwned<rustls::ClientConnection, TcpStream> {
    let mut roots = rustls::RootCertStore::empty();
    if trusted {
        roots.add(rustls::pki_types::CertificateDer::from(CA)).unwrap();
    }
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let client = rustls::ClientConnection::new(Arc::new(config), name.try_into().unwrap()).unwrap();
    let stream = TcpStream::connect(address).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(8))).unwrap();
    rustls::StreamOwned::new(client, stream)
}
fn raw(address: SocketAddr, request: &[u8]) -> String {
    let mut stream = tls(address, true, "localhost");
    stream.write_all(request).unwrap();
    let mut bytes = Vec::new();
    // A closed TLS connection without close_notify can report UnexpectedEof after
    // delivering the HTTP response; status/body are still checked below.
    let _ = stream.read_to_end(&mut bytes);
    String::from_utf8(bytes).unwrap()
}
fn rpc(address: SocketAddr, token: &str, request: Request) -> Response {
    let server_id = if matches!(&request, Request::ManagedCatalog) {
        None
    } else {
        match rpc(address, token, Request::ManagedCatalog) {
            Response::ManagedCatalog { catalog } => Some(catalog.server_id),
            // Preserve authentication failures without fabricating an identity.
            error => return error,
        }
    };
    let body = serde_json::to_vec(&serde_json::json!({"protocol":PROTOCOL,"server_id":server_id,"request":request})).unwrap();
    let mut wire = format!("POST {HTTP_RPC_PATH} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).into_bytes();
    wire.extend(body);
    let response = raw(address, &wire);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap()
}
#[test]
fn startup_requires_credentials_and_valid_tls() {
    let root = Temp::new();
    assert!(HttpsServer::start(Store::open(&root.0).unwrap(), config()).is_err());
    assert!(HttpsConfig::from_der("127.0.0.1:0".parse().unwrap(), vec![], KEY.to_vec()).is_err());
}
#[test]
fn occupied_listener_startup_releases_the_writer_and_store_lock() {
    let root = Temp::new();
    // Install the same valid trusted principal configuration as successful tests.
    server(&root).shutdown().unwrap();
    let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let config = HttpsConfig::from_der(occupied.local_addr().unwrap(), vec![CERT.to_vec()], KEY.to_vec()).unwrap();
    let start = std::time::Instant::now();
    assert!(HttpsServer::start(Store::open(&root.0).unwrap(), config).is_err());
    assert!(start.elapsed() < Duration::from_secs(5));
    // This would fail while a detached writer still owned the service store.
    assert!(Store::open(&root.0).is_ok());
}
#[test]
fn configured_roles_checkout_cas_and_receipt_survive_https() {
    let root = Temp::new();
    let server = server(&root);
    let address = server.address();
    assert!(matches!(rpc(address,VIEWER,Request::ManagedCreate{logical_path:"p/a.solidcraft".into()}),Response::Error{code} if code=="forbidden"));
    assert!(matches!(rpc(address,&"dd".repeat(32),Request::ManagedCatalog),Response::Error{code} if code=="unauthorized"));
    let entry = match rpc(address, ALICE, Request::ManagedCreate { logical_path: "p/a.solidcraft".into() }) {
        Response::ManagedDocument { entry } => entry,
        other => panic!("{other:?}"),
    };
    let checkout = |token| {
        rpc(address, token, Request::ManagedCheckout { document: entry.document.clone(), workspace: "work-a".into(), machine: "machine-a".into() })
    };
    let grant = match checkout(ALICE) {
        Response::ManagedCheckout { grant } => grant,
        other => panic!("{other:?}"),
    };
    assert!(matches!(checkout(BOB),Response::Error{code} if code=="conflict"));
    let mut input = ManagedCheckIn {
        document: entry.document.clone(),
        workspace: "work-a".into(),
        machine: "machine-a".into(),
        checkout_token: grant.token,
        request_id: "https-retry".into(),
        parent: Some(12),
        content: b"toy".to_vec(),
        dependencies: vec![],
        comment: "Toy HTTPS version".into(),
    };
    assert!(matches!(rpc(address,ALICE,Request::ManagedCheckIn{input:input.clone()}),Response::Error{code} if code=="conflict"));
    input.parent = None;
    let first = rpc(address, ALICE, Request::ManagedCheckIn { input: input.clone() });
    assert!(matches!(first, Response::ManagedCheckedIn { .. }));
    let retry = rpc(address, ALICE, Request::ManagedCheckIn { input });
    assert_eq!(serde_json::to_value(first).unwrap(), serde_json::to_value(retry).unwrap());
    server.shutdown().unwrap();
}
#[test]
fn tls_and_http_reject_invalid_inputs() {
    let root = Temp::new();
    let server = server(&root);
    let address = server.address();
    let client = solidcraft_pdm::client::Client::https_with_ca(
        &format!("https://localhost:{}", address.port()),
        solidcraft_pdm::AuthToken::parse(ALICE.into()).unwrap(),
        CA,
    )
    .unwrap();
    assert!(client.managed_catalog().is_ok());
    assert!(tls(address, false, "localhost").write_all(b"GET / HTTP/1.1\r\n\r\n").is_err());
    assert!(tls(address, true, "wrong.invalid").write_all(b"GET / HTTP/1.1\r\n\r\n").is_err());
    assert!(raw(address, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").starts_with("HTTP/1.1 405"));
    assert!(raw(address, b"POST /bad HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n").starts_with("HTTP/1.1 404"));
    let invalid = format!(
        "POST {HTTP_RPC_PATH} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {ALICE}\r\nContent-Type: application/json\r\nContent-Length: 1\r\n\r\nx"
    );
    assert!(raw(address, invalid.as_bytes()).starts_with("HTTP/1.1 400"));
    assert!(client.managed_catalog().is_ok());
    server.shutdown().unwrap();
}

#[test]
fn simultaneous_checkout_has_exactly_one_winner() {
    let root = Temp::new();
    let server = server(&root);
    let address = server.address();
    let document = match rpc(address, ALICE, Request::ManagedCreate { logical_path: "parts/race.solidcraft".into() }) {
        Response::ManagedDocument { entry } => entry.document,
        other => panic!("{other:?}"),
    };
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = [ALICE, BOB]
        .into_iter()
        .map(|token| {
            let barrier = barrier.clone();
            let document = document.clone();
            std::thread::spawn(move || {
                barrier.wait();
                rpc(address, token, Request::ManagedCheckout { document, workspace: "race".into(), machine: "test".into() })
            })
        })
        .collect();
    let responses: Vec<_> = threads.into_iter().map(|thread| thread.join().unwrap()).collect();
    assert_eq!(responses.iter().filter(|r| matches!(r, Response::ManagedCheckout { .. })).count(), 1);
    assert_eq!(responses.iter().filter(|r| matches!(r, Response::Error { code } if code == "conflict")).count(), 1);
    assert!(matches!(rpc(address, ALICE, Request::ManagedCatalog), Response::ManagedCatalog { .. }));
}

#[test]
fn configured_identity_fence_rejects_missing_and_stale_mutation_targets() {
    let root = Temp::new();
    let server = server(&root);
    let address = server.address();
    for (identity, expected) in [(None, "server_identity_required"), (Some("0".repeat(64)), "server_identity_mismatch")] {
        let body = serde_json::to_vec(&serde_json::json!({"protocol":PROTOCOL,"server_id":identity,"request":Request::ManagedCreate { logical_path:"parts/wrong-target.solidcraft".into() }})).unwrap();
        let mut wire = format!("POST {HTTP_RPC_PATH} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {ALICE}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).into_bytes();
        wire.extend(body);
        let response = raw(address, &wire);
        assert!(response.starts_with("HTTP/1.1 200"));
        let response: Response = serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert!(matches!(response,Response::Error{code} if code==expected));
    }
    match rpc(address, ALICE, Request::ManagedCatalog) {
        Response::ManagedCatalog { catalog } => assert!(catalog.entries.is_empty()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn chunked_body_cannot_bypass_the_size_limit() {
    let root = Temp::new();
    let server = server(&root);
    let address = server.address();
    let mut stream = tls(address, true, "localhost");
    let header = format!(
        "POST {HTTP_RPC_PATH} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {ALICE}\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n",
        solidcraft_pdm::MAX_FRAME + 1
    );
    stream.write_all(header.as_bytes()).unwrap();
    // The server may reject and close while the caller is still transmitting.
    let _ = stream.write_all(&vec![b'x'; solidcraft_pdm::MAX_FRAME as usize + 1]);
    let _ = stream.write_all(b"\r\n0\r\n\r\n");
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    assert!(response.starts_with(b"HTTP/1.1 413"), "{}", String::from_utf8_lossy(&response));
    assert!(matches!(rpc(address, ALICE, Request::ManagedCatalog), Response::ManagedCatalog { .. }));
}

#[test]
fn stalled_tls_and_body_are_closed_without_poisoning_the_service() {
    let root = Temp::new();
    let server = server(&root);
    let address = server.address();
    let mut stalled_tls = TcpStream::connect(address).unwrap();
    stalled_tls.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
    let start = std::time::Instant::now();
    let mut byte = [0];
    match stalled_tls.read(&mut byte) {
        Ok(0) => (),
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => (),
        result => panic!("TLS deadline did not close the connection: {result:?}"),
    }
    assert!(start.elapsed() < Duration::from_secs(8));
    let header = format!(
        "POST {HTTP_RPC_PATH} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {ALICE}\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\nx"
    );
    let start = std::time::Instant::now();
    assert!(raw(address, header.as_bytes()).starts_with("HTTP/1.1 408"));
    assert!(start.elapsed() < Duration::from_secs(8));
    assert!(matches!(rpc(address, ALICE, Request::ManagedCatalog), Response::ManagedCatalog { .. }));
}
