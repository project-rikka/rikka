use crate::login::LoginError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("malformed {0}")]
    Malformed(&'static str),

    #[error("failed to decode {subject}: {source}")]
    Decode {
        subject: &'static str,
        source: prost::DecodeError,
    },

    #[error("the access point failed the server key check")]
    Signature,

    #[error("packet authentication failed")]
    Mac,

    #[error("packet of {0} bytes exceeds the frame limit")]
    Oversized(usize),

    #[error("the access point refused the login: {0}")]
    LoginFailed(LoginError),

    #[error("invalid Spotify id {0:?}")]
    Id(String),
}

impl Error {
    pub(crate) fn decode(subject: &'static str) -> impl FnOnce(prost::DecodeError) -> Error {
        move |source| Error::Decode { subject, source }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
