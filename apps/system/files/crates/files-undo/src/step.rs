use files_core::Location;
use files_vfs::{Outcome, Placement};
use rikka_file_operations::TaskKind;

use crate::change::Change;
use crate::kind::Kind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub kind: Kind,
    pub change: Change,
}

impl Step {
    #[must_use]
    pub fn from_outcome(kind: TaskKind, outcome: &Outcome) -> Option<Self> {
        Some(Self {
            kind: Kind::from_task(kind)?,
            change: Change::from_outcome(kind, outcome)?,
        })
    }

    #[must_use]
    pub fn renamed(from: Location, to: Location) -> Self {
        Self {
            kind: Kind::Rename,
            change: Change::Moved(vec![Placement::new(from, to)]),
        }
    }

    #[must_use]
    pub fn new_folder(at: Location) -> Self {
        Self {
            kind: Kind::NewFolder,
            change: Change::Created(vec![at]),
        }
    }

    #[must_use]
    pub fn new_file(at: Location) -> Self {
        Self {
            kind: Kind::NewFile,
            change: Change::Created(vec![at]),
        }
    }
}
