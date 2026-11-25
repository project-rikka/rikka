use std::time::Duration;

#[derive(Debug, Clone, thiserror::Error)]
pub(crate) enum Error {
    #[error("Discord sign-in expired")]
    SignedOut,

    #[error("Discord refused: {0}")]
    Forbidden(String),

    #[error("Discord rejected request: {0}")]
    Rejected(String),

    #[error("Discord requires a captcha; finish it once in the Discord web app")]
    Captcha,

    #[error("Discord requires two-factor verification")]
    Mfa(discord_core::MfaChallenge),

    #[error("{0}")]
    Missing(String),

    #[error("{0}")]
    TooLarge(String),

    #[error("rate limited by Discord for {0:?}")]
    Throttled(Duration),

    #[error("Discord unreachable: {0}")]
    Unreachable(String),

    #[error("unexpected Discord response: {0}")]
    Unreadable(String),

    #[error("QR sign-in failed: {0}")]
    Handshake(String),

    #[error("voice failed: {0}")]
    Voice(String),

    #[error("cancelled")]
    Cancelled,

    #[error("{0}")]
    Storage(String),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn unreadable(error: impl std::fmt::Display) -> Error {
        Error::Unreadable(error.to_string())
    }

    pub(crate) fn unreachable(error: impl std::fmt::Display) -> Error {
        Error::Unreachable(error.to_string())
    }
}

impl From<Error> for discord_core::Error {
    fn from(error: Error) -> discord_core::Error {
        let message = error.to_string();
        match error {
            Error::SignedOut => discord_core::Error::Refused(message),
            Error::Forbidden(reason) | Error::Rejected(reason) => {
                discord_core::Error::Denied(reason)
            }
            Error::Missing(what) => discord_core::Error::Missing(what),
            Error::TooLarge(what) => discord_core::Error::TooLarge(what),
            Error::Throttled(wait) => discord_core::Error::Throttled(wait),
            Error::Unreachable(_) => discord_core::Error::Unreachable(message),
            Error::Unreadable(_) | Error::Handshake(_) => discord_core::Error::Unreadable(message),
            Error::Voice(reason) => discord_core::Error::Voice(reason),
            Error::Captcha => discord_core::Error::Captcha,
            Error::Mfa(challenge) => discord_core::Error::Mfa(challenge),
            Error::Cancelled => discord_core::Error::Cancelled,
            Error::Storage(reason) => discord_core::Error::Storage(reason),
        }
    }
}

impl From<discord_core::Error> for Error {
    fn from(error: discord_core::Error) -> Error {
        match error {
            discord_core::Error::Refused(_) => Error::SignedOut,
            discord_core::Error::Denied(reason) => Error::Forbidden(reason),
            discord_core::Error::Missing(what) => Error::Missing(what),
            discord_core::Error::Throttled(wait) => Error::Throttled(wait),
            discord_core::Error::TooLarge(what) => Error::TooLarge(what),
            discord_core::Error::Unreachable(reason) => Error::Unreachable(reason),
            discord_core::Error::Unreadable(reason) => Error::Unreadable(reason),
            discord_core::Error::Captcha => Error::Captcha,
            discord_core::Error::Mfa(challenge) => Error::Mfa(challenge),
            discord_core::Error::Cancelled => Error::Cancelled,
            discord_core::Error::Voice(reason) => Error::Voice(reason),
            discord_core::Error::Storage(reason) => Error::Storage(reason),
        }
    }
}
