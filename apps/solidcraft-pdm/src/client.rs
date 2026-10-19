//! Authenticated loopback client. The transport stays native and account-free.
use crate::{AuthToken, AuthenticatedRequest, Error, MAX_CONTENT, Request, Response, Version, VersionRef, read_socket_frame, write_socket_frame};
use std::{
    fs::File,
    io::Read,
    net::{Ipv4Addr, SocketAddrV4, TcpStream},
    path::Path,
    time::Duration,
};

#[derive(Clone)]
pub struct Client {
    port: u16,
    token: AuthToken,
}

impl Client {
    pub fn new(port: u16, token: AuthToken) -> Self {
        Self { port, token }
    }

    pub fn request(&self, request: Request) -> Result<Response, Error> {
        self.request_impl(request).map_err(|error| match error {
            Error::Io(error) => Error::Network(error),
            other => other,
        })
    }

    fn request_impl(&self, request: Request) -> Result<Response, Error> {
        let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port);
        let mut stream = TcpStream::connect_timeout(&address.into(), Duration::from_secs(5))?;
        let envelope = AuthenticatedRequest::new(&self.token, request);
        let bytes = serde_json::to_vec(&envelope)?;
        write_socket_frame(&mut stream, &bytes, Duration::from_secs(5))?;
        let response: Response = serde_json::from_slice(&read_socket_frame(&stream, Duration::from_secs(5))?)?;
        match response {
            Response::Error { code } => Err(Error::Server(code)),
            other => Ok(other),
        }
    }

    pub fn managed_catalog(&self) -> Result<crate::managed::ManagedCatalog, Error> {
        match self.request(Request::ManagedCatalog)? {
            Response::ManagedCatalog { catalog } => Ok(catalog),
            _ => Err(Error::Invalid("unexpected managed catalog response")),
        }
    }

    pub fn managed_create(&self, logical_path: String) -> Result<crate::managed::CatalogEntry, Error> {
        match self.request(Request::ManagedCreate { logical_path })? {
            Response::ManagedDocument { entry } => Ok(entry),
            _ => Err(Error::Invalid("unexpected managed create response")),
        }
    }

    pub fn managed_checkout(&self, document: String, workspace: String, machine: String) -> Result<crate::managed::CheckoutGrant, Error> {
        match self.request(Request::ManagedCheckout { document, workspace, machine })? {
            Response::ManagedCheckout { grant } => Ok(grant),
            _ => Err(Error::Invalid("unexpected managed checkout response")),
        }
    }

    pub fn managed_cancel_checkout(
        &self,
        document: String,
        workspace: String,
        machine: String,
        checkout_token: String,
    ) -> Result<crate::managed::CatalogEntry, Error> {
        match self.request(Request::ManagedCancelCheckout { document, workspace, machine, checkout_token })? {
            Response::ManagedDocument { entry } => Ok(entry),
            _ => Err(Error::Invalid("unexpected managed cancel response")),
        }
    }

    pub fn managed_check_in(&self, input: crate::managed::ManagedCheckIn) -> Result<crate::managed::ManagedVersion, Error> {
        match self.request(Request::ManagedCheckIn { input })? {
            Response::ManagedCheckedIn { version } => Ok(version),
            _ => Err(Error::Invalid("unexpected managed check-in response")),
        }
    }

    pub fn managed_history(&self, document: String) -> Result<Vec<crate::managed::ManagedVersion>, Error> {
        match self.request(Request::ManagedHistory { document })? {
            Response::ManagedHistory { versions } => Ok(versions),
            _ => Err(Error::Invalid("unexpected managed history response")),
        }
    }

    pub fn list(&self, document: Option<String>) -> Result<Vec<Version>, Error> {
        match self.request(Request::List { document })? {
            Response::Listed { versions } => Ok(versions),
            _ => Err(Error::Invalid("unexpected list response")),
        }
    }

    pub fn check_in_file(&self, document: String, parent: Option<u64>, path: &Path, dependencies: Vec<VersionRef>) -> Result<Version, Error> {
        let content = read_file(path, MAX_CONTENT)?;
        match self.request(Request::CheckIn { document, parent, content, dependencies })? {
            Response::CheckedIn { version } => Ok(version),
            _ => Err(Error::Invalid("unexpected check-in response")),
        }
    }

    pub fn download(&self, reference: &VersionRef) -> Result<(Version, Vec<u8>), Error> {
        match self.request(Request::Download { reference: reference.clone() })? {
            Response::Downloaded { version, content } => Ok((version, content)),
            _ => Err(Error::Invalid("unexpected download response")),
        }
    }

    pub fn retrieve(&self, reference: VersionRef, destination: &Path) -> Result<crate::workspace::Snapshot, Error> {
        crate::workspace::retrieve(reference, destination, |reference| self.download(reference))
    }
}

/// Read only a regular source file, with a bound even if it grows while being read.
pub fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>, Error> {
    if !std::fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(Error::Invalid("source must be a regular file"));
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Invalid("source must be a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(Error::Invalid("source exceeds size limit"));
    }
    Ok(bytes)
}
