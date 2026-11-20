use std::time::Duration;

use rikka_hifi_core::{DeviceId, SignalPath};

use crate::source::SourceId;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Started(SourceId),
    Advanced {
        from: SourceId,
        to: SourceId,
    },
    Position {
        source: SourceId,
        at: Duration,
    },
    Ended(SourceId),
    Paused,
    Resumed,
    Stopped,
    Path(Box<SignalPath>),
    Output(OutputState),
    Failed {
        source: Option<SourceId>,
        failure: Failure,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputState {
    Shared,
    Exclusive(DeviceId),
    FellBack { device: DeviceId, reason: Refusal },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("device not connected")]
    Missing,

    #[error("device is held by another application")]
    Busy { holder: Option<String> },

    #[error("device supports no format this source needs")]
    Unsupported,

    #[error("device disconnected")]
    Lost,

    #[error("device released to another application")]
    Released,

    #[error("failed to open device: {0}")]
    Open(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    #[error("failed to read source: {0}")]
    Unreadable(String),

    #[error("source is not seekable")]
    Unseekable,

    #[error("no output available: {0}")]
    Silent(String),
}
