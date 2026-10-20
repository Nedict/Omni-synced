//! Bounded HTTPS RPC transport. Only configured principals reach the writer.
pub use crate::http_listener::HttpsConfig;
use crate::http_listener::{Handler, HttpListener};
use crate::{Error, HTTP_RPC_PATH, MAX_FRAME, PROTOCOL, Request, Response, RpcEnvelope, Store};
use http_body_util::{BodyExt, Full, Limited};
use hyper::{
    Request as HttpRequest, Response as HttpResponse, StatusCode,
    body::{Bytes, Incoming},
};
use std::{
    net::{SocketAddr, TcpListener},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use tokio::sync::oneshot;
const DEADLINE: Duration = Duration::from_secs(5);
type Reply = HttpResponse<Full<Bytes>>;
type Job = (Request, String, Option<String>, oneshot::Sender<Response>);
pub struct HttpsServer {
    address: SocketAddr,
    listener: Option<HttpListener>,
    writer: Option<thread::JoinHandle<()>>,
}
impl HttpsServer {
    pub fn start(store: Store, config: HttpsConfig) -> Result<Self, Error> {
        Self::start_impl(store, config, None)
    }
    /// Keep an installer-selected loopback port reserved throughout enrollment.
    /// This consumes the socket and never closes/rebinds it during startup.
    pub fn start_loopback_with_listener(store: Store, config: HttpsConfig, listener: TcpListener) -> Result<Self, Error> {
        if !listener.local_addr()?.ip().is_loopback() {
            return Err(Error::Invalid("reserved HTTPS listener requires loopback"));
        }
        Self::start_impl(store, config, Some(listener))
    }
    fn start_impl(mut store: Store, config: HttpsConfig, reserved: Option<TcpListener>) -> Result<Self, Error> {
        if !store.has_configured_principals() {
            return Err(Error::Invalid("HTTPS requires configured principals"));
        }
        let (send, receive) = mpsc::sync_channel::<Job>(16);
        let writer = thread::Builder::new().name("pdm-writer".into()).spawn(move || {
            while let Ok((request, credential, expected, reply)) = receive.recv() {
                let response = store.execute_configured_for(request, &credential, expected.as_deref());
                let _ = reply.send(response);
            }
        })?;
        let handler: Handler = Arc::new(move |request| {
            let send = send.clone();
            Box::pin(async move {
                match rpc(request, send).await {
                    Ok(reply) => reply,
                    Err(never) => match never {},
                }
            })
        });
        let listener = match reserved {
            Some(listener) => HttpListener::start_with_listener(config, listener, handler, 16, Duration::from_secs(15)),
            None => HttpListener::start(config, handler, 16, Duration::from_secs(15)),
        };
        match listener {
            Ok(listener) => Ok(Self { address: listener.address(), listener: Some(listener), writer: Some(writer) }),
            Err(error) => {
                let _ = writer.join();
                Err(error)
            }
        }
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn shutdown(mut self) -> Result<(), Error> {
        self.close()
    }
    fn close(&mut self) -> Result<(), Error> {
        let listener_result = match self.listener.take() {
            Some(listener) => listener.shutdown(),
            None => Ok(()),
        };
        let writer_result = match self.writer.take() {
            Some(writer) => writer.join().map_err(|_| Error::Invalid("PDM writer failed")),
            None => Ok(()),
        };
        listener_result.and(writer_result)
    }
}
impl Drop for HttpsServer {
    fn drop(&mut self) {
        let _ = self.close();
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
    if send.try_send((envelope.request, credential, envelope.server_id, result)).is_err() {
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
