use crate::capability::{CapabilityId, Version};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SeamError {
    #[error("no service provides `{capability}`")]
    UnknownCapability { capability: CapabilityId },

    #[error("`{capability}` declares no method `{method}`")]
    UnknownMethod {
        capability: CapabilityId,
        method: String,
    },

    #[error("`{capability}` declares no event `{event}`")]
    UnknownEvent {
        capability: CapabilityId,
        event: String,
    },

    #[error("`{capability}` version {provided} does not satisfy {required}")]
    VersionMismatch {
        capability: CapabilityId,
        required: Version,
        provided: Version,
    },

    #[error("`{capability}` is already registered")]
    DuplicateCapability { capability: CapabilityId },

    #[error("`{name}` is not a valid routing key")]
    InvalidName { name: String },

    #[error("invalid parameters: {0}")]
    InvalidParams(String),

    #[error("failed to encode payload: {0}")]
    InvalidPayload(String),

    #[error("unavailable: {0}")]
    Unavailable(String),

    #[error("failed: {0}")]
    Failed(String),

    #[error("`{capability}/{method}` was dropped without a reply")]
    NoReply {
        capability: CapabilityId,
        method: String,
    },

    #[error("seam is not running")]
    Disconnected,

    #[error("seam is at capacity")]
    Overloaded,

    #[error("{message}")]
    Remote { code: u16, message: String },
}

impl SeamError {
    pub fn code(&self) -> u16 {
        match self {
            Self::UnknownCapability { .. } => 100,
            Self::UnknownMethod { .. } => 101,
            Self::UnknownEvent { .. } => 102,
            Self::VersionMismatch { .. } => 103,
            Self::DuplicateCapability { .. } => 200,
            Self::InvalidName { .. } => 201,
            Self::InvalidParams(_) => 300,
            Self::InvalidPayload(_) => 301,
            Self::Unavailable(_) => 400,
            Self::Failed(_) => 401,
            Self::NoReply { .. } => 402,
            Self::Disconnected => 500,
            Self::Overloaded => 501,
            Self::Remote { code, .. } => *code,
        }
    }

    pub fn unknown_method(capability: impl Into<CapabilityId>, method: impl Into<String>) -> Self {
        Self::UnknownMethod {
            capability: capability.into(),
            method: method.into(),
        }
    }

    pub fn invalid_params(reason: impl std::fmt::Display) -> Self {
        Self::InvalidParams(reason.to_string())
    }

    pub fn unavailable(reason: impl std::fmt::Display) -> Self {
        Self::Unavailable(reason.to_string())
    }

    pub fn failed(reason: impl std::fmt::Display) -> Self {
        Self::Failed(reason.to_string())
    }
}
