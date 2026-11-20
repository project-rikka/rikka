use std::path::PathBuf;

use rikka_proton_account::{FeedError, UserId};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("API error: {0}")]
    Api(Box<rikka_proton_api::Error>),

    #[error("cryptography error: {0}")]
    Pgp(Box<rikka_proton_pgp::Error>),

    #[error("account error: {0}")]
    Account(Box<rikka_proton_account::Error>),

    #[error("cache error: {0}")]
    Cache(Box<rikka_proton_cache::Error>),

    #[error("no signed-in account {0}")]
    NoSuchAccount(UserId),

    #[error("I/O error on {path}: {source}")]
    Storage {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid account list {0}")]
    InvalidAccountList(PathBuf),
}

impl Error {
    #[must_use]
    pub fn api(&self) -> Option<&rikka_proton_api::Error> {
        match self {
            Error::Api(error) => Some(error),
            Error::Account(error) => error.api(),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_signed_out(&self) -> bool {
        self.api()
            .is_some_and(rikka_proton_api::Error::is_signed_out)
    }
}

impl From<Error> for FeedError {
    fn from(error: Error) -> FeedError {
        FeedError::new(error, Error::api)
    }
}

macro_rules! boxed {
    ($($variant:ident($source:ty)),+ $(,)?) => {
        $(
            impl From<$source> for Error {
                fn from(error: $source) -> Error {
                    Error::$variant(Box::new(error))
                }
            }
        )+
    };
}

boxed!(
    Api(rikka_proton_api::Error),
    Pgp(rikka_proton_pgp::Error),
    Account(rikka_proton_account::Error),
    Cache(rikka_proton_cache::Error),
);

pub type Result<T, E = Error> = std::result::Result<T, E>;
