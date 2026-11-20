use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("XDG_RUNTIME_DIR is unset or not an absolute path")]
    NoRuntimeDir,
    #[error("failed to create nested display directory: {0}")]
    Directory(#[source] io::Error),
    #[error("failed to bind nested display socket: {0}")]
    Bind(#[source] io::Error),
    #[error("failed to spawn nested display thread: {0}")]
    Thread(#[source] io::Error),
    #[error("failed to start nested display: {0}")]
    Start(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
