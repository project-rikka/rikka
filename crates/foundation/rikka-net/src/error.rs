use std::error::Error as StdError;

use http::StatusCode;

use crate::checksum::ChecksumAlgorithm;
use crate::status_error::StatusError;

pub type Cause = Box<dyn StdError + Send + Sync>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("failed to connect")]
    Connect(#[source] Cause),

    #[error("connection interrupted")]
    Transport(#[source] Cause),

    #[error("request timed out")]
    Timeout,

    #[error("TLS handshake failed")]
    Tls(#[source] Cause),

    #[error(transparent)]
    Status(Box<StatusError>),

    #[error("too many redirects")]
    TooManyRedirects,

    #[error("failed to read response body")]
    Body(#[source] Cause),

    #[error("invalid {format} in response")]
    Decode {
        format: &'static str,
        #[source]
        source: Cause,
    },

    #[error("failed to encode {format}")]
    Encode {
        format: &'static str,
        #[source]
        source: Cause,
    },

    #[error("{0}")]
    InvalidHeader(String),

    #[error("request cancelled")]
    Cancelled,

    #[error("connection closed")]
    ConnectionClosed,

    #[error("send queue full")]
    Backpressure,

    #[error("protocol violation: {0}")]
    Protocol(String),

    #[error("malformed event stream: {0}")]
    EventStream(String),

    #[error("{0}")]
    InvalidChecksum(String),

    #[error("file exists: {}", .0.display())]
    AlreadyExists(std::path::PathBuf),

    #[error("{algorithm} checksum mismatch")]
    ChecksumMismatch {
        algorithm: ChecksumAlgorithm,
        expected: String,
        actual: String,
    },

    #[error("{0}")]
    Unsupported(String),

    #[error("response exceeds {limit} bytes")]
    TooLarge { limit: usize },

    #[error("giving up after {attempts} attempts")]
    RetriesExhausted {
        attempts: u32,
        #[source]
        source: Box<Error>,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    #[must_use]
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Error::Status(error) => Some(error.status()),
            Error::RetriesExhausted { source, .. } => source.status(),
            _ => None,
        }
    }

    #[must_use]
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self.innermost() {
            Error::Status(error) => error.retry_after(),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(self.innermost(), Error::Timeout)
    }

    #[must_use]
    pub fn is_connect(&self) -> bool {
        matches!(self.innermost(), Error::Connect(_))
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(self.innermost(), Error::Cancelled)
    }

    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self.innermost() {
            Error::Connect(_) | Error::Transport(_) | Error::Timeout => true,
            Error::Io(io) => matches!(
                io.kind(),
                std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::Interrupted
            ),
            Error::Status(error) => is_transient_status(error.status()),
            _ => false,
        }
    }

    fn innermost(&self) -> &Error {
        match self {
            Error::RetriesExhausted { source, .. } => source.innermost(),
            other => other,
        }
    }
}

pub(crate) fn exhausted(attempt: u32, error: Error) -> Error {
    if attempt == 0 {
        return error;
    }
    Error::RetriesExhausted {
        attempts: attempt + 1,
        source: Box::new(error),
    }
}

#[must_use]
pub(crate) fn is_transient_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_EARLY
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
            | StatusCode::INSUFFICIENT_STORAGE
    )
}

impl From<StatusError> for Error {
    fn from(error: StatusError) -> Self {
        Error::Status(Box::new(error))
    }
}

impl From<url::ParseError> for Error {
    fn from(error: url::ParseError) -> Self {
        Error::InvalidUrl(error.to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            return Error::Timeout;
        }
        if error.is_connect() {
            return classify_connect(error);
        }
        if error.is_redirect() {
            return Error::TooManyRedirects;
        }
        if error.is_builder() {
            return Error::InvalidUrl(error.to_string());
        }
        if error.is_decode() && !is_incomplete(&error) {
            return Error::Body(Box::new(error));
        }
        Error::Transport(Box::new(error))
    }
}

fn chain<'a>(
    error: &'a (dyn StdError + 'static),
) -> impl Iterator<Item = &'a (dyn StdError + 'static)> {
    std::iter::successors(Some(error), |current| StdError::source(*current))
}

fn is_incomplete(error: &reqwest::Error) -> bool {
    chain(error)
        .find_map(<dyn StdError + 'static>::downcast_ref::<std::io::Error>)
        .is_some_and(|io| {
            matches!(
                io.kind(),
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::BrokenPipe
            )
        })
}

fn classify_connect(error: reqwest::Error) -> Error {
    let tls = chain(&error).any(|current| {
        let text = current.to_string();
        text.contains("certificate")
            || text.contains("tls")
            || text.contains("TLS")
            || text.contains("handshake")
    });

    match tls {
        true => Error::Tls(Box::new(error)),
        false => Error::Connect(Box::new(error)),
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        use tokio_tungstenite::tungstenite::Error as WsError;

        match error {
            WsError::ConnectionClosed | WsError::AlreadyClosed => Error::ConnectionClosed,
            WsError::Io(io) => Error::Io(io),
            WsError::Tls(tls) => Error::Tls(Box::new(tls)),
            WsError::Url(url) => Error::InvalidUrl(url.to_string()),
            WsError::Capacity(capacity) => Error::Protocol(capacity.to_string()),
            WsError::Protocol(protocol) => Error::Protocol(protocol.to_string()),
            WsError::AttackAttempt => Error::Protocol("frame too large".to_owned()),
            WsError::WriteBufferFull(_) => Error::Protocol("write buffer full".to_owned()),
            WsError::Utf8(_) => Error::Protocol("text frame is not valid UTF-8".to_owned()),
            other @ (WsError::Http(_) | WsError::HttpFormat(_)) => {
                Error::Transport(Box::new(other))
            }
        }
    }
}
