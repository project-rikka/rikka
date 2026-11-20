use std::convert::Infallible;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::client::SeamClient;
use crate::dispatcher::Dispatcher;
use crate::error::SeamError;
use crate::event::EventQueue;
use crate::manifest::Manifest;

const NO_SESSION: &str = "no seam socket: this platform has no Rikka session";

#[derive(Debug)]
pub struct SocketClient {
    never: Infallible,
}

impl SocketClient {
    pub fn from_env() -> io::Result<Self> {
        Err(io::Error::new(io::ErrorKind::NotFound, NO_SESSION))
    }

    pub fn for_capability(capability: &str) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("nothing in this session serves `{capability}`: {NO_SESSION}"),
        ))
    }

    pub fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("cannot reach {}: {NO_SESSION}", path.as_ref().display()),
        ))
    }

    pub fn manifests(&self) -> Result<Vec<Manifest>, SeamError> {
        match self.never {}
    }

    #[must_use]
    pub fn client(&self) -> SeamClient {
        match self.never {}
    }

    #[must_use]
    pub fn dispatcher(&self) -> Arc<dyn Dispatcher> {
        match self.never {}
    }

    #[must_use]
    pub fn events(&self) -> EventQueue {
        match self.never {}
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        match self.never {}
    }

    pub async fn closed(&self) {
        match self.never {}
    }
}

#[derive(Debug)]
pub struct SocketListener {
    never: Infallible,
}

impl SocketListener {
    pub fn serve(path: impl Into<PathBuf>, client: SeamClient) -> io::Result<Self> {
        drop(client);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("cannot serve {}: {NO_SESSION}", path.into().display()),
        ))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        match self.never {}
    }
}

impl Drop for SocketListener {
    fn drop(&mut self) {
        match self.never {}
    }
}
