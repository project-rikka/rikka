use std::time::Duration;

use rikka_net::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Account(#[from] google_account::Error),

    #[error(transparent)]
    Net(#[from] rikka_net::Error),

    #[error("the sync server answered {0}")]
    Status(StatusCode),

    #[error("the sync server no longer accepts this sign-in")]
    Unauthorized,

    #[error("malformed sync message: {0}")]
    Decode(#[from] prost::DecodeError),

    #[error("failed to compress the sync message: {0}")]
    Compress(std::io::Error),

    #[error("the sync server asks to wait {0:?}")]
    Throttled(Duration),

    #[error("the sync server is busy")]
    Transient,

    #[error("sync is turned off for this account")]
    Disabled,

    #[error("the sync server refused the request: {0}")]
    Refused(String),

    #[error("the sync server started over, local sync state was reset")]
    StoreReset,

    #[error("sync protocol violation: {0}")]
    Protocol(&'static str),

    #[error("unreadable sync state: {0}")]
    Corrupt(&'static str),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    #[must_use]
    pub fn is_signed_out(&self) -> bool {
        match self {
            Error::Unauthorized => true,
            Error::Account(error) => error.is_signed_out(),
            _ => false,
        }
    }
}
