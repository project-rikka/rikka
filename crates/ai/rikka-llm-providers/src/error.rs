use crate::backend::Backend;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("{0} has no API key")]
    Keyless(Backend),

    #[error("{0} is not signed in")]
    SignedOut(Backend),

    #[error("model request failed: {0}")]
    Refused(String),

    #[error("{0} uses an API key, not sign-in")]
    Unsignable(Backend),

    #[error("{0} keeps no catalogue to pick models from")]
    Uncatalogued(Backend),

    #[error("{0} does not list the models of an account")]
    Unlisted(Backend),

    #[error("{0} does not report usage")]
    Unmetered(Backend),

    #[error(transparent)]
    SignIn(rikka_llm::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn from_adapter(backend: Backend, error: rikka_llm_adapter::Error) -> Error {
        match error {
            rikka_llm_adapter::Error::Keyless => Error::Keyless(backend),
            rikka_llm_adapter::Error::SignedOut => Error::SignedOut(backend),
            rikka_llm_adapter::Error::Refused(reason) => Error::Refused(reason),
        }
    }
}
