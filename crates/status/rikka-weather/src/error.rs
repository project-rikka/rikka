#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("weather service is unreachable: {0}")]
    Unreachable(String),
    #[error("weather service rejected the request: {0}")]
    Refused(String),
    #[error("invalid weather service response: {0}")]
    Unreadable(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<rikka_net::Error> for Error {
    fn from(error: rikka_net::Error) -> Error {
        match error {
            rikka_net::Error::Status(status) => Error::Refused(status.to_string()),
            rikka_net::Error::Decode { .. } => Error::Unreadable(error.to_string()),
            other => Error::Unreachable(other.to_string()),
        }
    }
}
