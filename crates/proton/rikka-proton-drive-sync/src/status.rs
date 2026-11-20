use std::path::{Path, PathBuf};

use jiff::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Starting,
    Scanning,
    Syncing { done: usize, total: usize },
    Idle,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemKind {
    Transfer,
    InvalidName,
    Unreadable,
    LocalFolderMissing,
    RemoteFolderRemoved,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    path: PathBuf,
    kind: ProblemKind,
    reason: String,
}

impl Problem {
    pub(crate) fn new(
        path: impl Into<PathBuf>,
        kind: ProblemKind,
        reason: impl Into<String>,
    ) -> Problem {
        Problem {
            path: path.into(),
            kind,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn kind(&self) -> &ProblemKind {
        &self.kind
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub(crate) activity: Activity,
    pub(crate) problems: Vec<Problem>,
    pub(crate) conflicts: Vec<PathBuf>,
    pub(crate) online: bool,
    pub(crate) synced_at: Option<Timestamp>,
}

impl Status {
    pub(crate) fn starting() -> Status {
        Status {
            activity: Activity::Starting,
            problems: Vec::new(),
            conflicts: Vec::new(),
            online: true,
            synced_at: None,
        }
    }

    #[must_use]
    pub fn activity(&self) -> Activity {
        self.activity
    }

    #[must_use]
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    #[must_use]
    pub fn conflicts(&self) -> &[PathBuf] {
        &self.conflicts
    }

    #[must_use]
    pub fn is_online(&self) -> bool {
        self.online
    }

    #[must_use]
    pub fn synced_at(&self) -> Option<Timestamp> {
        self.synced_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemState {
    Syncing(Direction),
    Transferred { direction: Direction, bytes: u64 },
    Synced,
    Failed(String),
    Conflicted { copy: PathBuf },
    Trashed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemUpdate {
    path: PathBuf,
    state: ItemState,
}

impl ItemUpdate {
    pub(crate) fn new(path: impl Into<PathBuf>, state: ItemState) -> ItemUpdate {
        ItemUpdate {
            path: path.into(),
            state,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn state(&self) -> &ItemState {
        &self.state
    }
}
