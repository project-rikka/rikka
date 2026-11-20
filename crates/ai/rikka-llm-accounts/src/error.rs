#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("no model backend configured")]
    Unconfigured,

    #[error("{0} has no API key")]
    Keyless(String),

    #[error("{0} is not signed in")]
    SignedOut(String),

    #[error("no model selected for {0}")]
    Modelless(String),

    #[error("unknown model \"{named}\"; available: {offered}")]
    Unoffered { named: String, offered: String },

    #[error("model request failed: {0}")]
    Refused(String),

    #[error("{0} does not report usage")]
    Unmetered(String),

    #[error("sign-in cancelled")]
    Stopped,
}

impl From<rikka_llm_providers::Error> for Error {
    fn from(error: rikka_llm_providers::Error) -> Error {
        use rikka_llm_providers::Error as Provider;

        match error {
            Provider::Keyless(backend) => Error::Keyless(backend.label().to_owned()),
            Provider::SignedOut(backend) => Error::SignedOut(backend.label().to_owned()),
            Provider::Unmetered(backend) => Error::Unmetered(backend.label().to_owned()),
            Provider::Refused(refused) => Error::Refused(refused),
            other => Error::Refused(other.to_string()),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
