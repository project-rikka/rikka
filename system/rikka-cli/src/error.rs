use std::io;

use rikka_seam::SeamError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("no Rikka session found ({0})")]
    NoSession(String),

    #[error("{0}")]
    NoProvider(io::Error),

    #[error("session ended")]
    SessionEnded,

    #[error("cannot spawn watcher thread: {0}")]
    WatchThread(io::Error),

    #[error("invalid JSON: {0}")]
    InvalidJson(SeamError),

    #[error("no wallpaper `{0}`")]
    NoSuchWallpaper(String),

    #[error(transparent)]
    Seam(#[from] SeamError),

    #[error(transparent)]
    Settings(#[from] rikka_settings::Error),

    #[error("cannot write output")]
    Output(#[from] io::Error),

    #[error("cannot write JSON")]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub(crate) fn is_closed_output(&self) -> bool {
        match self {
            Self::Output(error) => error.kind() == io::ErrorKind::BrokenPipe,
            Self::Json(error) => error.io_error_kind() == Some(io::ErrorKind::BrokenPipe),
            _ => false,
        }
    }
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
