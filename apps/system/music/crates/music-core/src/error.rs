#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("service unreachable: {0}")]
    Unreachable(String),

    #[error("service rejected account: {0}")]
    Refused(String),

    #[error("sign-in was cancelled")]
    Cancelled,

    #[error("account cannot stream: {0}")]
    NotEntitled(String),

    #[error("{0} was not found")]
    Missing(String),

    #[error("unexpected service response: {0}")]
    Unreadable(String),

    #[error("not supported: {0}")]
    Unsupported(String),

    #[error("playback failed: {0}")]
    NoOutput(String),

    #[error("cannot save account: {0}")]
    Storage(String),

    #[error("cannot update music cache: {0}")]
    Disk(String),

    #[error("failed to spawn thread: {0}")]
    Spawn(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn unreachable(error: impl std::fmt::Display) -> Error {
        Error::Unreachable(error.to_string())
    }

    #[must_use]
    pub fn from_status(status: u16, message: String) -> Error {
        match status {
            401 | 403 => Error::Refused(message),
            404 => Error::Missing(message),
            429 | 500..600 => Error::Unreachable(message),
            _ => Error::Unreadable(message),
        }
    }
}

impl From<rikka_account_book::Error> for Error {
    fn from(error: rikka_account_book::Error) -> Error {
        Error::Storage(error.to_string())
    }
}
