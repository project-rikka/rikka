use std::path::PathBuf;

use rikka_proton_client::Error as ProtonError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("proton: {0}")]
    Proton(#[from] ProtonError),

    #[error("account: {0}")]
    Account(#[from] rikka_proton_account::Error),

    #[error("cannot resolve state directory: {0}")]
    Paths(#[from] rikka_dirs::Error),

    #[error("cannot access {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("cache: {0}")]
    Cache(#[from] rikka_proton_cache::Error),

    #[error("keyring: {0}")]
    Keyring(#[from] rikka_secrets::Error),

    #[error("sign-in through {0} is not supported")]
    IdentityProvider(String),

    #[error("unsupported sign-in method")]
    UnsupportedSignIn,
}

impl Error {
    #[must_use]
    pub fn is_signed_out(&self) -> bool {
        self.api()
            .is_some_and(rikka_proton_api::Error::is_signed_out)
    }

    #[must_use]
    pub fn is_unreachable(&self) -> bool {
        matches!(
            self.api(),
            Some(
                rikka_proton_api::Error::Network(_)
                    | rikka_proton_api::Error::NoRoute
                    | rikka_proton_api::Error::TooManyRequests { .. }
            )
        )
    }

    fn api(&self) -> Option<&rikka_proton_api::Error> {
        match self {
            Error::Account(error) => error.api(),
            Error::Proton(error) => error.api(),
            _ => None,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
