use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("XDG_RUNTIME_DIR is unset or not an absolute path")]
    NoRuntimeDir,
    #[error("socket path {0} exceeds 107 bytes")]
    PathTooLong(PathBuf),
    #[error("{0} is locked by another compositor")]
    SocketLocked(PathBuf),
    #[error("no free socket name between wayland-0 and wayland-32")]
    NoFreeSocket,
    #[error("{action}: {source}")]
    Io {
        action: String,
        #[source]
        source: io::Error,
    },
}

impl Error {
    pub(crate) fn io(action: impl Into<String>) -> impl FnOnce(io::Error) -> Self {
        let action = action.into();
        move |source| Self::Io { action, source }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
