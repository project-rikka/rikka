use rikka_net::StatusCode;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Account(#[from] google_account::Error),

    #[error(transparent)]
    Net(#[from] rikka_net::Error),

    #[error("Drive no longer accepts this sign-in")]
    Unauthorized,

    #[error("the item is no longer in Drive")]
    Missing,

    #[error("Drive does not allow this: {0}")]
    Forbidden(String),

    #[error("Drive storage is full")]
    StorageFull,

    #[error("Drive asks to slow down")]
    RateLimited,

    #[error("Drive answered {status}: {message}")]
    Api { status: StatusCode, message: String },

    #[error("malformed answer from Drive: {0}")]
    Malformed(&'static str),

    #[error("this item has no file content to download")]
    NotDownloadable,

    #[error("failed to read {path}: {source}")]
    File {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error(transparent)]
    Cache(#[from] rikka_account_book::Error),
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
    #[serde(default)]
    errors: Vec<Detail>,
}

#[derive(Deserialize)]
struct Detail {
    #[serde(default)]
    reason: String,
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

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Error::Net(error) if error.is_cancelled())
    }
    pub(crate) fn from_answer(status: StatusCode, body: &str) -> Error {
        let failure = serde_json::from_str::<Envelope>(body)
            .map(|envelope| envelope.error)
            .ok();
        let message = failure
            .as_ref()
            .map(|failure| failure.message.clone())
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| body.chars().take(200).collect());
        let reason = failure
            .as_ref()
            .and_then(|failure| failure.errors.first())
            .map_or("", |detail| detail.reason.as_str());
        match (status, reason) {
            (StatusCode::UNAUTHORIZED, _) => Error::Unauthorized,
            (StatusCode::NOT_FOUND, _) => Error::Missing,
            (_, "storageQuotaExceeded" | "teamDriveFileLimitExceeded") => Error::StorageFull,
            (StatusCode::TOO_MANY_REQUESTS, _)
            | (_, "rateLimitExceeded" | "userRateLimitExceeded") => Error::RateLimited,
            (StatusCode::FORBIDDEN, _) => Error::Forbidden(message),
            _ => Error::Api { status, message },
        }
    }
}
