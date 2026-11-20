use rikka_nix_log::{Count, Tally};
use serde::{Deserialize, Serialize};
use zvariant::Type;

use crate::error::Error;
use crate::request::{Action, ChangeState};
use crate::run::{Operation, Phase, Run, RunId, RunState, Status, millis, time};

const UNKNOWN_GENERATION: u32 = 0;

const NOT_FINISHED: u64 = 0;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RunRecord {
    pub id: u64,
    pub operation: String,
    pub action: String,
    pub target: u32,
    pub started: u64,
    pub finished: u64,
    pub state: String,
    pub generation: u32,
    pub reason: String,
    pub unchanged: bool,
    pub status: StatusRecord,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct StatusRecord {
    pub phase: String,
    pub builds: CountRecord,
    pub paths: CountRecord,
    pub copied: CountRecord,
    pub downloaded: CountRecord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CountRecord {
    pub done: u64,
    pub expected: u64,
    pub running: u64,
    pub failed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PageRecord {
    pub lines: Vec<String>,
    pub next: u64,
    pub end: bool,
}

impl From<&Run> for RunRecord {
    fn from(run: &Run) -> Self {
        let (action, target) = match run.operation {
            Operation::Apply { action } | Operation::Update { action } => (action.name(), 0),
            Operation::Rollback { generation } => ("", generation),
        };
        let (state, generation, reason, unchanged) = match &run.state {
            RunState::Running => ("running", UNKNOWN_GENERATION, String::new(), true),
            RunState::Done { generation } => (
                "done",
                generation.unwrap_or(UNKNOWN_GENERATION),
                String::new(),
                false,
            ),
            RunState::Failed { reason, changes } => (
                "failed",
                UNKNOWN_GENERATION,
                reason.clone(),
                changes.is_untouched(),
            ),
            RunState::Interrupted => ("interrupted", UNKNOWN_GENERATION, String::new(), false),
        };
        Self {
            id: run.id.get(),
            operation: run.operation.name().to_owned(),
            action: action.to_owned(),
            target,
            started: millis(run.started),
            finished: run.finished.map_or(NOT_FINISHED, millis),
            state: state.to_owned(),
            generation,
            reason,
            unchanged,
            status: StatusRecord::from(run.status),
        }
    }
}

impl TryFrom<RunRecord> for Run {
    type Error = Error;

    fn try_from(record: RunRecord) -> Result<Self, Error> {
        let action = || {
            Action::from_name(&record.action)
                .ok_or_else(|| malformed(format!("unknown action `{}`", record.action)))
        };
        let operation = match record.operation.as_str() {
            "apply" => Operation::Apply { action: action()? },
            "update" => Operation::Update { action: action()? },
            "rollback" => Operation::Rollback {
                generation: record.target,
            },
            other => return Err(malformed(format!("unknown operation `{other}`"))),
        };
        let state = match record.state.as_str() {
            "running" => RunState::Running,
            "done" => RunState::Done {
                generation: (record.generation != UNKNOWN_GENERATION).then_some(record.generation),
            },
            "failed" => RunState::Failed {
                reason: record.reason,
                changes: ChangeState::from_unchanged(record.unchanged),
            },
            "interrupted" => RunState::Interrupted,
            other => return Err(malformed(format!("unknown run state `{other}`"))),
        };
        Ok(Self {
            id: RunId::new(record.id),
            operation,
            started: time(record.started),
            finished: (record.finished != NOT_FINISHED).then(|| time(record.finished)),
            state,
            status: Status::try_from(record.status)?,
        })
    }
}

impl From<Status> for StatusRecord {
    fn from(status: Status) -> Self {
        Self {
            phase: status.phase.name().to_owned(),
            builds: status.tally.builds.into(),
            paths: status.tally.paths.into(),
            copied: status.tally.copied.into(),
            downloaded: status.tally.downloaded.into(),
        }
    }
}

impl TryFrom<StatusRecord> for Status {
    type Error = Error;

    fn try_from(record: StatusRecord) -> Result<Self, Error> {
        let phase = Phase::from_name(&record.phase)
            .ok_or_else(|| malformed(format!("unknown phase `{}`", record.phase)))?;
        Ok(Self {
            phase,
            tally: Tally {
                builds: record.builds.into(),
                paths: record.paths.into(),
                copied: record.copied.into(),
                downloaded: record.downloaded.into(),
            },
        })
    }
}

impl From<Count> for CountRecord {
    fn from(count: Count) -> Self {
        Self {
            done: count.done,
            expected: count.expected,
            running: count.running,
            failed: count.failed,
        }
    }
}

impl From<CountRecord> for Count {
    fn from(record: CountRecord) -> Self {
        Self {
            done: record.done,
            expected: record.expected,
            running: record.running,
            failed: record.failed,
        }
    }
}

fn malformed(reason: String) -> Error {
    Error::Malformed(reason)
}
