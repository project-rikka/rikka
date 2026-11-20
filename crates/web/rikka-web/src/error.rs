use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Gecko engine not installed")]
    NotInstalled,
    #[error("unusable profile directory {path}: {source}")]
    Profile { path: PathBuf, source: io::Error },
    #[error("failed to start Gecko engine: {0}")]
    Spawn(io::Error),
    #[error("failed to start engine display: {0}")]
    Display(String),
    #[error("engine exited")]
    EngineExited,
    #[error("session closed")]
    Closed,
    #[error("engine rejected the request: {0}")]
    Rejected(String),
    #[error("engine dropped the request without a reply")]
    NoReply,
    #[error("unexpected engine reply: {0}")]
    Malformed(String),
    #[error("{0} is not an absolute path")]
    RelativePath(PathBuf),
    #[error("invalid saved session state: {0}")]
    State(serde_json::Error),
    #[error("time precedes the Unix epoch")]
    BeforeEpoch,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
