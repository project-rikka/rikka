use std::fmt;

use rikka_nix_protocol::ChangeState;

use crate::error::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Denied,
    Busy,
    Stale,
    Invalid(String),
    Unavailable(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied => Error::Denied.fmt(f),
            Self::Busy => f.write_str("another change is already being applied"),
            Self::Stale => f.write_str("the system configuration changed since it was read"),
            Self::Invalid(reason) | Self::Unavailable(reason) => f.write_str(reason),
        }
    }
}

impl From<&Error> for Refusal {
    fn from(error: &Error) -> Self {
        match error {
            Error::Denied => Self::Denied,
            Error::Unavailable(reason) => Self::Unavailable(reason.clone()),
            other => Self::Invalid(other.to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Done {
        generation: Option<u32>,
    },
    Failed {
        reason: String,
        changes: ChangeState,
    },
    Refused(Refusal),
}

impl Outcome {
    #[must_use]
    pub fn changes(&self) -> ChangeState {
        match self {
            Self::Done { .. } => ChangeState::Modified,
            Self::Failed { changes, .. } => *changes,
            Self::Refused(_) => ChangeState::Untouched,
        }
    }

    #[must_use]
    pub fn refusal(&self) -> Option<&Refusal> {
        match self {
            Self::Refused(Refusal::Denied) | Self::Done { .. } | Self::Failed { .. } => None,
            Self::Refused(refusal) => Some(refusal),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    Started,
    Finished(Outcome),
}
