use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Google Chrome is not installed")]
    ChromeNotInstalled,
    #[error("rikka-web-host is not installed")]
    HostNotInstalled,
    #[error("unusable profile directory {path}: {source}")]
    Profile { path: PathBuf, source: io::Error },
    #[error("failed to open the extension channel: {0}")]
    Channel(io::Error),
    #[error("failed to start Google Chrome: {0}")]
    Spawn(io::Error),
    #[error("failed to start the engine display: {0}")]
    Display(String),
    #[error("Google Chrome exited")]
    EngineExited,
    #[error("the Rikka extension disconnected")]
    Disconnected,
    #[error("session closed")]
    Closed,
    #[error("Chrome rejected the request: {0}")]
    Rejected(String),
    #[error("unexpected reply from Chrome: {0}")]
    Malformed(String),
    #[error("{0} is not an absolute path")]
    RelativePath(PathBuf),
    #[error("failed to prepare Chrome's private mounts at {path}: {source}")]
    Isolation { path: PathBuf, source: io::Error },
    #[error("{0} is missing, so Rikka's Chrome policy cannot apply")]
    PolicyMountMissing(PathBuf),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
