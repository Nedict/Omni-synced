//! Shared bounded HTTPS listener. No document or relay authority lives here.
use crate::Error;
use http_body_util::Full;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    service::service_fn,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use tokio::sync::{Semaphore, oneshot};
const DEADLINE: Duration = Duration::from_secs(5);
pub(crate) type Reply = Response<Full<Bytes>>;
pub(crate) type Handler = Arc<dyn Fn(Request<Incoming>) -> Pin<Box<dyn Future<Output = Reply> + Send>> + Send + Sync>;
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

pub(crate) struct HttpListener {
    address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl HttpListener {
    pub(crate) fn start(config: HttpsConfig, handler: Handler, connections: usize, lifetime: Duration) -> Result<Self, Error> {
        if connections == 0 || connections > 32 || lifetime > Duration::from_secs(20) || lifetime.is_zero() {
            return Err(Error::Invalid("HTTPS listener limits"));
        }
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().max_blocking_threads(32).build()?;
        let listener = std::net::TcpListener::bind(config.bind)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let (stop, mut stopped) = oneshot::channel();
        let (ready, readiness) = mpsc::sync_channel(1);
        let thread = thread::Builder::new().name("pdm-https-listener".into()).spawn(move || {
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
            if ready.send(Ok(())).is_err() {
                return;
            }
            runtime.block_on(async move {
                let acceptor = tokio_rustls::TlsAcceptor::from(config.tls);
                let permits = Arc::new(Semaphore::new(connections));
                loop {
                    if !matches!(stopped.try_recv(), Err(oneshot::error::TryRecvError::Empty)) {
                        break;
                    }
                    match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
                        Ok(Ok((stream, _))) => {
                            let Ok(permit) = permits.clone().try_acquire_owned() else { continue };
                            let acceptor = acceptor.clone();
                            let handler = handler.clone();
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
                                let service = service_fn(move |request| {
                                    let future = handler(request);
                                    async move { Ok::<_, std::convert::Infallible>(future.await) }
                                });
                                let _ = tokio::time::timeout(lifetime, builder.serve_connection(TokioIo::new(tls), service)).await;
                            });
                        }
                        Ok(Err(_)) => break,
                        Err(_) => continue,
                    }
                }
            });
            // Blocking handlers have explicit <=10s deadlines; shutdown waits for them.
            drop(runtime);
        })?;
        let mut listener = Self { address, stop: Some(stop), thread: Some(thread) };
        match readiness.recv_timeout(DEADLINE) {
            Ok(Ok(())) => Ok(listener),
            Ok(Err(error)) => {
                let _ = listener.close();
                Err(Error::Io(error))
            }
            Err(_) => {
                let _ = listener.close();
                Err(Error::Invalid("HTTPS startup failed"))
            }
        }
    }
    pub(crate) fn address(&self) -> SocketAddr {
        self.address
    }
    pub(crate) fn shutdown(mut self) -> Result<(), Error> {
        self.close()
    }
    fn close(&mut self) -> Result<(), Error> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| Error::Invalid("HTTPS worker failed"))?;
        }
        Ok(())
    }
}
impl Drop for HttpListener {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
