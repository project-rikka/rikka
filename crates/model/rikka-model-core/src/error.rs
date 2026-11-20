use crate::options::Limit;
use crate::requirement::Requirement;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("no importer or exporter handles {0}")]
    UnknownFormat(String),

    #[error("corrupt model: {0}")]
    Damaged(String),

    #[error("{format} version {version} is not supported")]
    UnsupportedVersion {
        format: &'static str,
        version: String,
    },

    #[error("unsupported required extensions: {}", .0.join(", "))]
    RequiredExtensions(Vec<String>),

    #[error("invalid model: {0}")]
    Invalid(String),

    #[error("{format} needs {requirement}")]
    Unmet {
        format: &'static str,
        requirement: Requirement,
    },

    #[error("failed to encode {what}: {reason}")]
    Encode { what: &'static str, reason: String },

    #[error("{limit} limit exceeded: {actual} of {allowed}")]
    LimitsExceeded {
        limit: Limit,
        actual: u64,
        allowed: u64,
    },

    #[error("failed to read \"{uri}\": {reason}")]
    Read { uri: String, reason: String },

    #[error("failed to write \"{uri}\": {reason}")]
    Write { uri: String, reason: String },

    #[error("cancelled")]
    Cancelled,
}

impl Error {
    pub(crate) fn read(uri: &str, reason: impl Into<String>) -> Self {
        Self::Read {
            uri: uri.to_owned(),
            reason: reason.into(),
        }
    }

    pub(crate) fn write(uri: &str, reason: impl Into<String>) -> Self {
        Self::Write {
            uri: uri.to_owned(),
            reason: reason.into(),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
