//! Bounded HTTPS RPC transport. Only configured principals reach the writer.
use crate::{Error, HTTP_RPC_PATH, MAX_FRAME, PROTOCOL, Request, Response, RpcEnvelope, Store};
use http_body_util::{BodyExt, Full, Limited};
use hyper::{
    Request as HttpRequest, Response as HttpResponse, StatusCode,
    body::{Bytes, Incoming},
    service::service_fn,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use std::{
    net::SocketAddr,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use tokio::sync::{Semaphore, oneshot};

const DEADLINE: Duration = Duration::from_secs(5);
type Reply = HttpResponse<Full<Bytes>>;
type Job = (Request, String, oneshot::Sender<Response>);

pub struct HttpsConfig {
    bind: SocketAddr,
    tls: Arc<rustls::ServerConfig>,
}

impl HttpsConfig {
    pub fn from_der(bind: SocketAddr, certificates: Vec<Vec<u8>>, key: Vec<u8>) -> Result<Self, Error> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let tls = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|_| Error::Invalid("TLS versions"))?
            .with_no_client_auth()
            .with_single_cert(certificates.into_iter().map(Into::into).collect(), rustls::pki_types::PrivatePkcs8KeyDer::from(key).into())
            .map_err(|_| Error::Invalid("TLS certificate or private key"))?;
        Ok(Self { bind, tls: Arc::new(tls) })
    }
}

pub struct HttpsServer {
    address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl HttpsServer {
    pub fn start(mut store: Store, config: HttpsConfig) -> Result<Self, Error> {
        if !store.has_configured_principals() {
            return Err(Error::Invalid("HTTPS requires configured principals"));
        }
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let listener = std::net::TcpListener::bind(config.bind)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let (stop, mut stopped) = oneshot::channel();
        let (ready, readiness) = mpsc::sync_channel(1);
        let handle = thread::Builder::new().name("pdm-https".into()).spawn(move || {
            let listener = {
                let _entered = runtime.enter();
                match tokio::net::TcpListener::from_std(listener) {
                    Ok(listener) => listener,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                }
            };
            let (send, receive) = mpsc::sync_channel::<Job>(16);
            let writer = thread::Builder::new().name("pdm-writer".into()).spawn(move || {
                while let Ok((request, credential, reply)) = receive.recv() {
                    let response = store.execute_configured(request, &credential);
                    let _ = reply.send(response);
                }
            });
            let writer = match writer {
                Ok(writer) => writer,
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            if ready.send(Ok(())).is_err() {
                drop(send);
                let _ = writer.join();
                return;
            }
            {
                runtime.block_on(async move {
                    let acceptor = tokio_rustls::TlsAcceptor::from(config.tls);
                    let permits = Arc::new(Semaphore::new(16));
                    loop {
                        if !matches!(stopped.try_recv(), Err(oneshot::error::TryRecvError::Empty)) {
                            break;
                        }
                        match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
                            Ok(accepted) => {
                                let Ok((stream, _)) = accepted else { break };
                                let Ok(permit) = permits.clone().try_acquire_owned() else { continue };
                                let acceptor = acceptor.clone();
                                let send = send.clone();
                                tokio::spawn(async move {
                                    let _permit = permit;
                                    let Ok(Ok(tls)) = tokio::time::timeout(DEADLINE, acceptor.accept(stream)).await else { return };
                                    let mut builder = hyper::server::conn::http1::Builder::new();
                                    builder
                                        .timer(TokioTimer::new())
                                        .header_read_timeout(DEADLINE)
                                        .max_headers(32)
                                        .max_buf_size(16 * 1024)
                                        .keep_alive(false);
                                    let service = service_fn(move |request| rpc(request, send.clone()));
                                    let _ = tokio::time::timeout(Duration::from_secs(15), builder.serve_connection(TokioIo::new(tls), service)).await;
                                });
                            }
                            Err(_) => continue,
                        }
                    }
                });
                // Runtime drop cancels network tasks and releases all queue senders.
                drop(runtime);
                let _ = writer.join();
            }
        })?;
        let mut server = Self { address, stop: Some(stop), thread: Some(handle) };
        match readiness.recv_timeout(DEADLINE) {
            Ok(Ok(())) => Ok(server),
            Ok(Err(error)) => {
                let _ = server.stop_and_join();
                Err(Error::Io(error))
            }
            Err(_) => {
                let _ = server.stop_and_join();
                Err(Error::Invalid("HTTPS startup failed"))
            }
        }
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn shutdown(mut self) -> Result<(), Error> {
        self.stop_and_join()
    }
    fn stop_and_join(&mut self) -> Result<(), Error> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(handle) = self.thread.take() {
            handle.join().map_err(|_| Error::Invalid("HTTPS worker failed"))?;
        }
        Ok(())
    }
}
impl Drop for HttpsServer {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
    }
}

fn reply(status: StatusCode, body: impl Into<Bytes>) -> Reply {
    let mut response = HttpResponse::new(Full::new(body.into()));
    *response.status_mut() = status;
    response.headers_mut().insert(hyper::header::CONTENT_TYPE, hyper::header::HeaderValue::from_static("application/json"));
    response
}

async fn rpc(request: HttpRequest<Incoming>, send: mpsc::SyncSender<Job>) -> Result<Reply, std::convert::Infallible> {
    if request.method() != hyper::Method::POST {
        return Ok(reply(StatusCode::METHOD_NOT_ALLOWED, "{}"));
    }
    if request.uri().path() != HTTP_RPC_PATH || request.uri().query().is_some() {
        return Ok(reply(StatusCode::NOT_FOUND, "{}"));
    }
    if request.headers().contains_key(hyper::header::UPGRADE)
        || request.headers().get_all(hyper::header::AUTHORIZATION).iter().count() != 1
        || request.headers().get(hyper::header::CONTENT_TYPE).and_then(|h| h.to_str().ok()) != Some("application/json")
    {
        return Ok(reply(StatusCode::BAD_REQUEST, "{}"));
    }
    let credential = request.headers().get(hyper::header::AUTHORIZATION).and_then(|h| h.to_str().ok()).and_then(|h| h.strip_prefix("Bearer "));
    let Some(credential) = credential.filter(|c| c.len() == 64 && c.bytes().all(|b| b.is_ascii_hexdigit())).map(str::to_owned) else {
        return Ok(reply(StatusCode::UNAUTHORIZED, "{}"));
    };
    if request
        .headers()
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.parse::<u64>().ok())
        .is_some_and(|n| n > MAX_FRAME)
    {
        return Ok(reply(StatusCode::PAYLOAD_TOO_LARGE, "{}"));
    }
    let bytes = match tokio::time::timeout(DEADLINE, Limited::new(request.into_body(), MAX_FRAME as usize).collect()).await {
        Ok(Ok(body)) => body.to_bytes(),
        Ok(Err(_)) => return Ok(reply(StatusCode::PAYLOAD_TOO_LARGE, "{}")),
        Err(_) => return Ok(reply(StatusCode::REQUEST_TIMEOUT, "{}")),
    };
    let envelope: RpcEnvelope = match serde_json::from_slice(&bytes) {
        Ok(envelope) => envelope,
        Err(_) => return Ok(reply(StatusCode::BAD_REQUEST, "{}")),
    };
    if envelope.protocol != PROTOCOL {
        return Ok(reply(StatusCode::BAD_REQUEST, "{}"));
    }
    let (result, receive) = oneshot::channel();
    if send.try_send((envelope.request, credential, result)).is_err() {
        return Ok(reply(StatusCode::SERVICE_UNAVAILABLE, "{}"));
    }
    match tokio::time::timeout(DEADLINE, receive).await {
        Ok(Ok(response)) => match serde_json::to_vec(&response) {
            Ok(bytes) if bytes.len() <= MAX_FRAME as usize => Ok(reply(StatusCode::OK, bytes)),
            _ => Ok(reply(StatusCode::INTERNAL_SERVER_ERROR, "{}")),
        },
        _ => Ok(reply(StatusCode::SERVICE_UNAVAILABLE, "{}")),
    }
}
