use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::names::Conflict;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum Operation {
    Link {
        local: u64,
        remote: String,
    },
    Rebind {
        synced: i64,
        local: Option<u64>,
        remote: Option<String>,
    },
    Refresh {
        synced: i64,
    },
    Forget {
        synced: i64,
    },
    SetAside {
        local: u64,
        name: String,
        conflict: Conflict,
        forget: Option<i64>,
    },
    MoveRemote {
        synced: i64,
        parent: String,
        name: String,
    },
    MoveLocal {
        synced: i64,
        parent: u64,
        name: String,
    },
    CreateRemoteFolder {
        local: u64,
        parent: String,
        name: String,
    },
    CreateLocalFolder {
        remote: String,
        parent: u64,
        name: String,
    },
    TrashRemote {
        synced: i64,
    },
    TrashLocal {
        synced: i64,
    },
    Upload {
        local: u64,
        parent: String,
        name: String,
        sha1: String,
    },
    UploadRevision {
        synced: i64,
        sha1: String,
    },
    Download {
        remote: String,
        parent: u64,
        name: String,
        revision: String,
    },
    DownloadRevision {
        synced: i64,
        revision: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    Bookkeeping,
    Move,
    Create,
    Trash,
    Transfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
    Both,
}

impl Operation {
    #[must_use]
    pub fn phase(&self) -> Phase {
        match self {
            Operation::Link { .. }
            | Operation::Rebind { .. }
            | Operation::Refresh { .. }
            | Operation::Forget { .. }
            | Operation::SetAside { .. } => Phase::Bookkeeping,
            Operation::MoveRemote { .. } | Operation::MoveLocal { .. } => Phase::Move,
            Operation::CreateRemoteFolder { .. } | Operation::CreateLocalFolder { .. } => {
                Phase::Create
            }
            Operation::TrashRemote { .. } | Operation::TrashLocal { .. } => Phase::Trash,
            Operation::Upload { .. }
            | Operation::UploadRevision { .. }
            | Operation::Download { .. }
            | Operation::DownloadRevision { .. } => Phase::Transfer,
        }
    }

    #[must_use]
    pub fn direction(&self) -> Direction {
        match self {
            Operation::MoveRemote { .. }
            | Operation::CreateRemoteFolder { .. }
            | Operation::TrashRemote { .. }
            | Operation::Upload { .. }
            | Operation::UploadRevision { .. } => Direction::Up,
            Operation::MoveLocal { .. }
            | Operation::CreateLocalFolder { .. }
            | Operation::TrashLocal { .. }
            | Operation::Download { .. }
            | Operation::DownloadRevision { .. }
            | Operation::SetAside { .. } => Direction::Down,
            Operation::Link { .. }
            | Operation::Rebind { .. }
            | Operation::Refresh { .. }
            | Operation::Forget { .. } => Direction::Both,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Context {
    device: String,
    today: Date,
}

impl Context {
    #[must_use]
    pub fn new(device: impl Into<String>, today: Date) -> Context {
        Context {
            device: device.into(),
            today,
        }
    }

    #[must_use]
    pub fn device(&self) -> &str {
        &self.device
    }

    #[must_use]
    pub fn today(&self) -> Date {
        self.today
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    operations: Vec<Operation>,
}

impl Plan {
    pub(crate) fn new(operations: Vec<Operation>) -> Plan {
        Plan { operations }
    }

    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    #[must_use]
    pub fn into_operations(self) -> Vec<Operation> {
        self.operations
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.operations.len()
    }
}
