use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("cannot read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("cannot write {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("{file}:{line}: {message}")]
    Parse {
        file: String,
        line: usize,
        message: String,
    },
    #[error("{0}")]
    Data(String),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
