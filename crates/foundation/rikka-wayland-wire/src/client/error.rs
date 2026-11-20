use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::WireError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError {
    pub interface: &'static str,
    pub object: u32,
    pub code: u32,
    pub message: String,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "protocol error {} on {}#{}: {}",
            self.code, self.interface, self.object, self.message
        )
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("neither WAYLAND_SOCKET nor WAYLAND_DISPLAY names a display")]
    NoDisplay,
    #[error("WAYLAND_DISPLAY is relative and XDG_RUNTIME_DIR is not set")]
    NoRuntimeDir,
    #[error("WAYLAND_SOCKET does not name an open socket")]
    InheritedSocket,
    #[error("failed to connect to {path}: {source}")]
    Connect {
        path: PathBuf,
        source: Arc<io::Error>,
    },
    #[error("connection failed: {0}")]
    Io(Arc<io::Error>),
    #[error("malformed message from the compositor: {0}")]
    Wire(Arc<WireError>),
    #[error("{0}")]
    Protocol(ProtocolError),
    #[error("the compositor closed the connection")]
    Closed,
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(Arc::new(error))
    }
}

impl From<rustix::io::Errno> for Error {
    fn from(error: rustix::io::Errno) -> Self {
        Self::Io(Arc::new(error.into()))
    }
}

impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        match error {
            WireError::Io(error) => Self::Io(Arc::new(error)),
            error => Self::Wire(Arc::new(error)),
        }
    }
}
