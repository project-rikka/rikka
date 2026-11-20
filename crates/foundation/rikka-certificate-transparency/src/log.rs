use std::fmt;
use std::sync::Arc;

use base64::prelude::{BASE64_STANDARD, Engine as _};
use jiff::Timestamp;

use crate::public_key::PublicKey;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogId([u8; 32]);

impl LogId {
    pub(crate) fn new(bytes: [u8; 32]) -> LogId {
        LogId(bytes)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for LogId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "LogId({self})")
    }
}

impl fmt::Display for LogId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&BASE64_STANDARD.encode(self.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogState {
    Pending,
    Qualified,
    Usable,
    ReadOnly,
    Retired(Timestamp),
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Current,
    Retired,
}

impl LogState {
    pub(crate) fn standing(self, earliest_sct: Timestamp) -> Option<Standing> {
        match self {
            LogState::Qualified | LogState::Usable | LogState::ReadOnly => Some(Standing::Current),
            LogState::Retired(at) => (earliest_sct < at).then_some(Standing::Retired),
            LogState::Pending | LogState::Rejected => None,
        }
    }

    pub(crate) fn was_qualified(self) -> bool {
        !matches!(self, LogState::Pending | LogState::Rejected)
    }
}

#[derive(Debug, Clone)]
pub struct Log {
    pub(crate) id: LogId,
    pub(crate) description: String,
    pub(crate) operator: Arc<str>,
    pub(crate) key: PublicKey,
    pub(crate) state: LogState,
}

impl Log {
    #[must_use]
    pub fn id(&self) -> LogId {
        self.id
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn operator(&self) -> &str {
        &self.operator
    }

    #[must_use]
    pub fn state(&self) -> LogState {
        self.state
    }
}
