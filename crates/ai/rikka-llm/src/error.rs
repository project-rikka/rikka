use std::time::Duration;

pub type Cause = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("no API key given and {0} is not set")]
    MissingKey(&'static str),

    #[error("API key rejected")]
    Unauthorized,

    #[error("account verification required")]
    Unverified { url: String },

    #[error("quota exhausted or request exceeds account limits")]
    QuotaExceeded,

    #[error("rate limited")]
    RateLimited { retry_after: Option<Duration> },

    #[error("provider overloaded")]
    Overloaded,

    #[error("conversation exceeds the model's context window")]
    ContextOverflow,

    #[error("unknown model \"{0}\"")]
    UnknownModel(String),

    #[error("provider rejected the request: {0}")]
    Rejected(String),

    #[error("model refused: {0}")]
    Declined(String),

    #[error("{provider} does not support {feature}")]
    Unsupported {
        provider: &'static str,
        feature: &'static str,
    },

    #[error("request cancelled")]
    Cancelled,

    #[error("request timed out")]
    Timeout,

    #[error("provider unreachable")]
    Connect(#[source] Cause),

    #[error("connection interrupted")]
    Transport(#[source] Cause),

    #[error("invalid provider response: {0}")]
    Malformed(String),

    #[error(transparent)]
    Tool(#[from] rikka_tool::Error),

    #[error("response does not match the requested JSON schema")]
    Format(#[source] Cause),

    #[error("invalid endpoint \"{0}\"")]
    InvalidEndpoint(String),

    #[error("sign-in failed: {0}")]
    Login(String),

    #[error("credentials expired and cannot be renewed")]
    Expired,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Error::RateLimited { .. }
                | Error::Overloaded
                | Error::Timeout
                | Error::Connect(_)
                | Error::Transport(_)
        )
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Error::Cancelled)
    }

    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Error::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }

    #[must_use]
    pub fn verification_url(&self) -> Option<&str> {
        match self {
            Error::Unverified { url } => Some(url),
            _ => None,
        }
    }
}
