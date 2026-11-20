use std::time::Duration;

use http::StatusCode;

use crate::code::ResponseCode;
use crate::details::{DeviceVerificationDetails, HumanVerificationDetails};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("application version is missing")]
    MissingAppVersion,

    #[error("invalid application version \"{0}\", expected e.g. \"linux-pass@1.0.0\"")]
    InvalidAppVersion(String),

    #[error("invalid API URL \"{0}\"")]
    InvalidUrl(String),

    #[error("invalid HTTP header name or value")]
    InvalidHeader,

    #[error("human verification required")]
    HumanVerificationRequired(HumanVerificationDetails),

    #[error("device verification required")]
    DeviceVerificationRequired(DeviceVerificationDetails),

    #[error("session is missing scopes {}", .0.join(", "))]
    MissingScope(Vec<String>),

    #[error("application version is no longer supported")]
    ForceUpdate,

    #[error("too many requests")]
    TooManyRequests { retry_after: Option<Duration> },

    #[error("unauthorized")]
    Unauthorized,

    #[error("session expired")]
    SessionExpired,

    #[error("no session")]
    MissingSession,

    #[error("{}", .0.message)]
    Api(Box<ApiError>),

    #[error("API returned {status}")]
    Status { status: StatusCode, body: String },

    #[error(transparent)]
    Network(#[from] rikka_net::Error),

    #[error("invalid JSON response to {request}: {source}")]
    Decode {
        request: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to serialize request body: {0}")]
    Encode(#[source] serde_json::Error),

    #[error("invalid multipart form: {0}")]
    InvalidForm(String),

    #[error("session storage I/O failed: {0}")]
    Storage(#[source] std::io::Error),

    #[error("API and alternative routes unreachable")]
    NoRoute,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: ResponseCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl Error {
    #[must_use]
    pub fn code(&self) -> Option<ResponseCode> {
        match self {
            Error::Api(refusal) => Some(refusal.code),
            Error::HumanVerificationRequired(_) => Some(ResponseCode::HumanVerificationRequired),
            Error::DeviceVerificationRequired(_) => Some(ResponseCode::DeviceVerificationRequired),
            _ => None,
        }
    }

    #[must_use]
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Error::Status { status, .. } => Some(*status),
            Error::Api(refusal) => Some(refusal.status),
            Error::Unauthorized => Some(StatusCode::UNAUTHORIZED),
            Error::TooManyRequests { .. } => Some(StatusCode::TOO_MANY_REQUESTS),
            Error::Network(error) => error.status(),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_signed_out(&self) -> bool {
        matches!(
            self,
            Error::Unauthorized | Error::SessionExpired | Error::MissingSession
        )
    }

    #[must_use]
    pub fn is_unreachable(&self) -> bool {
        match self {
            Error::NoRoute => true,
            Error::Network(error) => error.is_connect() || error.is_timeout(),
            _ => false,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        Error::Storage(error)
    }
}
