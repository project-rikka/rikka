use crate::failure::Failure;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Api(#[from] rikka_proton_api::Error),

    #[error(transparent)]
    Account(#[from] rikka_proton_account::Error),

    #[error(transparent)]
    Pgp(#[from] rikka_proton_pgp::Error),

    #[error("key transparency failed: {0}")]
    Failed(#[from] Failure),

    #[error(transparent)]
    LogList(#[from] rikka_certificate_transparency::Error),

    #[error("invalid email address \"{0}\"")]
    InvalidEmail(String),

    #[error("API response has no epoch")]
    NoEpoch,

    #[error("no unlocked primary user key")]
    NoUserKey,

    #[error("invalid key transparency state JSON: {0}")]
    State(#[source] serde_json::Error),
}

impl Error {
    pub(crate) fn is_unprocessable(&self) -> bool {
        matches!(self, Error::Api(error) if error.status().is_some_and(|status| status.as_u16() == 422))
    }
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
}
