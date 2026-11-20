use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("cannot write {}: {source}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid package index: {0}")]
    Index(#[source] serde_json::Error),

    #[error("package index lists no packages")]
    EmptyIndex,

    #[error("cannot read package details: {0}")]
    Details(#[source] io::Error),

    #[error("invalid package details: {0}")]
    DetailsFormat(#[source] serde_json::Error),

    #[error("catalog import cancelled")]
    Cancelled,
}

impl Error {
    pub(crate) fn read(path: impl Into<PathBuf>) -> impl Fn(io::Error) -> Error {
        let path = path.into();
        move |source| Error::Read {
            path: path.clone(),
            source,
        }
    }

    pub(crate) fn write(path: impl Into<PathBuf>) -> impl Fn(io::Error) -> Error {
        let path = path.into();
        move |source| Error::Write {
            path: path.clone(),
            source,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
