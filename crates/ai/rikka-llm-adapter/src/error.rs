#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no API key given")]
    Keyless,

    #[error("account is not signed in")]
    SignedOut,

    #[error("model request failed: {0}")]
    Refused(String),
}

impl From<rikka_llm::Error> for Error {
    fn from(error: rikka_llm::Error) -> Error {
        match error {
            rikka_llm::Error::MissingKey(_) => Error::Keyless,
            refused => Error::Refused(refused.to_string()),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
