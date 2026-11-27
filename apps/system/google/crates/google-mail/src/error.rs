use rikka_net::StatusCode;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Account(#[from] google_account::Error),

    #[error(transparent)]
    Net(#[from] rikka_net::Error),

    #[error("Gmail no longer accepts this sign-in")]
    Unauthorized,

    #[error("the message is no longer in Gmail")]
    Missing,

    #[error("Gmail asks to slow down")]
    RateLimited,

    #[error("the mailbox changed too much since the last check")]
    HistoryExpired,

    #[error("Gmail answered {status}: {message}")]
    Api { status: StatusCode, message: String },

    #[error("malformed answer from Gmail: {0}")]
    Malformed(&'static str),

    #[error("{0} is not a mail address")]
    Address(String),

    #[error("a message needs at least one recipient")]
    NoRecipient,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Deserialize)]
struct Envelope {
    error: Failure,
}

#[derive(Deserialize)]
struct Failure {
    #[serde(default)]
    message: String,
}

impl Error {
    #[must_use]
    pub fn is_signed_out(&self) -> bool {
        match self {
            Error::Unauthorized => true,
            Error::Account(error) => error.is_signed_out(),
            _ => false,
        }
    }

    pub(crate) fn from_answer(status: StatusCode, body: &str) -> Error {
        let message = serde_json::from_str::<Envelope>(body)
            .map(|envelope| envelope.error.message)
            .ok()
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| body.chars().take(200).collect());
        match status {
            StatusCode::UNAUTHORIZED => Error::Unauthorized,
            StatusCode::NOT_FOUND => Error::Missing,
            StatusCode::TOO_MANY_REQUESTS => Error::RateLimited,
            _ => Error::Api { status, message },
        }
    }
}
