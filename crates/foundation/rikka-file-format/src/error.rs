use std::io;
use std::path::PathBuf;

use crate::version::VERSION;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("not a {expected} file")]
    Foreign { expected: &'static str },

    #[error("{format} version {found} is not supported; this build reads version {VERSION}")]
    Version { format: &'static str, found: u32 },

    #[error("{0}")]
    Malformed(&'static str),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to write {}: {source}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to encode {format} file: {source}")]
    Encode {
        format: &'static str,
        #[source]
        source: serde_json::Error,
    },

    #[error("malformed {format} file: {source}")]
    Json {
        format: &'static str,
        #[source]
        source: serde_json::Error,
    },

    #[error(transparent)]
    Decode(#[from] DecodeError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
