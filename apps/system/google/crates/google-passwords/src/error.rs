#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sync(#[from] google_sync::Error),

    #[error(transparent)]
    Account(#[from] google_account::Error),

    #[error(transparent)]
    Storage(#[from] rikka_account_book::Error),

    #[error("failed to read saved passwords: {0}")]
    Read(std::io::Error),

    #[error("saved passwords cannot be opened with this key")]
    Sealed,

    #[error(transparent)]
    Format(#[from] rikka_file_format::DecodeError),

    #[error("the site address is not a web address")]
    Address,

    #[error("a password cannot be empty")]
    EmptyPassword,

    #[error("the password is no longer saved")]
    Missing,

    #[error("failed to save passwords: {0}")]
    Task(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    #[must_use]
    pub fn is_signed_out(&self) -> bool {
        match self {
            Error::Sync(error) => error.is_signed_out(),
            Error::Account(error) => error.is_signed_out(),
            _ => false,
        }
    }
}
