#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("{path}: {reason}")]
    Unwritable { path: String, reason: String },

    #[error("{path}: {reason}")]
    Unreadable { path: String, reason: String },

    #[error("failed to set up assistant: {0}")]
    Setup(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("{0}")]
    Unattachable(String),

    #[error("the conversation is already answering")]
    Answering,

    #[error("the conversation has no assistant yet")]
    Unequipped,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn setup_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
    Error::Setup(error.into())
}
