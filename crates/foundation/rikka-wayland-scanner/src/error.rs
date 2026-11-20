use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("{path} is not well-formed XML: {source}")]
    Xml {
        path: PathBuf,
        source: rikka_xml::Error,
    },
    #[error("{path}: {message}")]
    Invalid { path: PathBuf, message: String },
    #[error("{0}")]
    Inconsistent(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
