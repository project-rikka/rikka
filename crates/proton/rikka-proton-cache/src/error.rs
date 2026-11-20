use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("unusable cache directory {path}: {source}")]
    Folder {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to open cache database: {0}")]
    Database(String),

    #[error("failed to derive cache key: {0}")]
    Key(#[from] rikka_proton_pgp::Error),

    #[error("failed to write cache: {0}")]
    Write(String),

    #[error("cache writer stopped")]
    Stopped,
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Error {
        Error::Database(error.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
