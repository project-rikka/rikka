use std::time::Duration;

use crate::manage::mfa::MfaChallenge;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("Discord unreachable: {0}")]
    Unreachable(String),

    #[error("signed out by Discord: {0}")]
    Refused(String),

    #[error("forbidden by Discord: {0}")]
    Denied(String),

    #[error("{0} was not found")]
    Missing(String),

    #[error("rate limited by Discord for {0:?}")]
    Throttled(Duration),

    #[error("{0} exceeds Discord's upload limit")]
    TooLarge(String),

    #[error("unexpected Discord response: {0}")]
    Unreadable(String),

    #[error("Discord requires a captcha; finish it once in the Discord web app")]
    Captcha,

    #[error("Discord requires two-factor verification")]
    Mfa(MfaChallenge),

    #[error("cancelled")]
    Cancelled,

    #[error("voice failed: {0}")]
    Voice(String),

    #[error("cannot save account: {0}")]
    Storage(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn unreachable(error: impl std::fmt::Display) -> Error {
        Error::Unreachable(error.to_string())
    }

    pub fn unreadable(error: impl std::fmt::Display) -> Error {
        Error::Unreadable(error.to_string())
    }

    pub fn voice(error: impl std::fmt::Display) -> Error {
        Error::Voice(error.to_string())
    }

    #[must_use]
    pub fn signs_out(&self) -> bool {
        matches!(self, Error::Refused(_))
    }
}

impl From<rikka_account_book::Error> for Error {
    fn from(error: rikka_account_book::Error) -> Error {
        Error::Storage(error.to_string())
    }
}
