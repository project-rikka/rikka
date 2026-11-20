#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("authenticator: {0}")]
    Authenticator(#[from] rikka_proton_authenticator::Error),

    #[error("invalid code")]
    NotACode,

    #[error("a code needs a title")]
    Untitled,

    #[error("a code needs a secret")]
    NoSecret,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
