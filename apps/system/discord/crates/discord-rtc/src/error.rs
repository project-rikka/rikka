#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Error {
    #[error("invalid voice endpoint: {0}")]
    Endpoint(String),

    #[error("voice server unreachable: {0}")]
    Unreachable(String),

    #[error("voice server timed out")]
    Timeout,

    #[error("unreadable voice server message: {0}")]
    Unreadable(String),

    #[error("no supported voice encryption mode")]
    NoMode,

    #[error("voice server did not report external address")]
    Discovery,

    #[error("voice server rejected token")]
    Rejected,

    #[error("voice session invalidated")]
    SessionInvalid,

    #[error("voice server closed connection ({code}): {reason}")]
    Closed { code: u16, reason: String },

    #[error("end-to-end encryption failed: {0}")]
    Dave(String),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn unreachable(error: impl std::fmt::Display) -> Error {
        Error::Unreachable(error.to_string())
    }

    pub(crate) fn unreadable(error: impl std::fmt::Display) -> Error {
        Error::Unreadable(error.to_string())
    }

    pub(crate) fn dave(error: impl std::fmt::Display) -> Error {
        Error::Dave(error.to_string())
    }
}

impl From<Error> for discord_core::Error {
    fn from(error: Error) -> discord_core::Error {
        match error {
            Error::Unreachable(_) | Error::Timeout | Error::Discovery => {
                discord_core::Error::unreachable(error)
            }
            Error::Unreadable(_) => discord_core::Error::unreadable(error),
            Error::Endpoint(_)
            | Error::NoMode
            | Error::Rejected
            | Error::SessionInvalid
            | Error::Closed { .. }
            | Error::Dave(_) => discord_core::Error::voice(error),
        }
    }
}
