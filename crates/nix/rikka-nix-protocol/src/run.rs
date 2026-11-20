use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rikka_nix_log::Tally;

use crate::request::{Action, ChangeState, Request};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunId(u64);

impl RunId {
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for RunId {
    type Err = ParseIntError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.parse().map(Self)
    }
}

impl From<SystemTime> for RunId {
    fn from(started: SystemTime) -> Self {
        Self(millis(started))
    }
}

pub(crate) fn millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |since| {
        u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
    })
}

pub(crate) fn time(millis: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(millis)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Apply { action: Action },
    Update { action: Action },
    Rollback { generation: u32 },
}

impl Operation {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Apply { .. } => "apply",
            Self::Update { .. } => "update",
            Self::Rollback { .. } => "rollback",
        }
    }

    #[must_use]
    pub fn action(self) -> Option<Action> {
        match self {
            Self::Apply { action } | Self::Update { action } => Some(action),
            Self::Rollback { .. } => None,
        }
    }
}

impl From<&Request> for Operation {
    fn from(request: &Request) -> Self {
        match request {
            Request::Apply { action, .. } => Self::Apply { action: *action },
            Request::Update { action } => Self::Update { action: *action },
            Request::Rollback { generation } => Self::Rollback {
                generation: *generation,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Preparing,
    Locking,
    Evaluating,
    Building,
    Activating,
}

impl Phase {
    pub const ALL: [Self; 5] = [
        Self::Preparing,
        Self::Locking,
        Self::Evaluating,
        Self::Building,
        Self::Activating,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Locking => "locking",
            Self::Evaluating => "evaluating",
            Self::Building => "building",
            Self::Activating => "activating",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|phase| phase.name() == name)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub phase: Phase,
    pub tally: Tally,
}

impl Status {
    #[must_use]
    pub fn fraction(self) -> Option<f32> {
        match self.phase {
            Phase::Building => self.tally.fraction(),
            Phase::Preparing | Phase::Locking | Phase::Evaluating | Phase::Activating => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunState {
    Running,
    Done {
        generation: Option<u32>,
    },
    Failed {
        reason: String,
        changes: ChangeState,
    },
    Interrupted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub id: RunId,
    pub operation: Operation,
    pub started: SystemTime,
    pub finished: Option<SystemTime>,
    pub state: RunState,
    pub status: Status,
}

impl Run {
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state == RunState::Running
    }

    pub fn interrupt(&mut self, at: SystemTime) {
        if self.is_running() {
            self.state = RunState::Interrupted;
            self.finished = Some(at);
        }
    }
}
