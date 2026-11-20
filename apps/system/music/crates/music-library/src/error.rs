use std::path::PathBuf;

use crate::ids::TrackId;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("cannot open music library: {0}")]
    Open(String),

    #[error("music library database error: {0}")]
    Database(String),

    #[error("{} is not an absolute folder path", .0.display())]
    Folder(PathBuf),

    #[error("track {0} is not in the music library")]
    UnknownTrack(TrackId),

    #[error("scan cancelled")]
    Cancelled,

    #[error("music library stopped")]
    Stopped,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Error {
        Error::Database(error.to_string())
    }
}
