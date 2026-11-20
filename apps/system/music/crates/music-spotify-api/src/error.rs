#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to reach Spotify: {0}")]
    Unreachable(#[source] rikka_net::Error),

    #[error("Spotify answered {status} to {endpoint}")]
    Status { status: u16, endpoint: String },

    #[error("failed to decode Spotify's {subject}: {source}")]
    Decode {
        subject: &'static str,
        source: prost::DecodeError,
    },

    #[error("malformed Spotify {0}")]
    Malformed(&'static str),

    #[error("Spotify refused the {flow} request: {reason}")]
    TokenRefused { flow: &'static str, reason: String },

    #[error("Spotify asks to retry the {0} request later")]
    Throttled(&'static str),

    #[error("could not answer Spotify's {0} challenge")]
    Challenge(&'static str),

    #[error("Spotify keeps the audio file {0} outside its CDN")]
    Storage(String),
}

impl Error {
    pub(crate) fn decode(subject: &'static str) -> impl FnOnce(prost::DecodeError) -> Error {
        move |source| Error::Decode { subject, source }
    }

    pub(crate) fn from_net(endpoint: &str) -> impl FnOnce(rikka_net::Error) -> Error {
        move |error| match error.status() {
            Some(status) => Error::Status {
                status: status.as_u16(),
                endpoint: endpoint.to_owned(),
            },
            None => Error::Unreachable(error),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
