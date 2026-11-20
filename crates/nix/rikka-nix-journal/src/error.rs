use std::io;
use std::path::PathBuf;

use rikka_nix_protocol::RunId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("malformed run record {path}: {reason}")]
    Record { path: PathBuf, reason: String },
    #[error("no run {0} is kept")]
    UnknownRun(RunId),
    #[error("byte {offset} does not start a line of run {run}")]
    Offset { run: RunId, offset: u64 },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
