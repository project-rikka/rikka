use files_core::Location;
use files_vfs::{Outcome, Placement, Task};
use rikka_file_operations::TaskKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Moved(Vec<Placement>),
    Copied(Vec<Placement>),
    Trashed(Vec<Placement>),
    Restored(Vec<Placement>),
    Created(Vec<Location>),
}

impl Change {
    #[must_use]
    pub fn from_outcome(kind: TaskKind, outcome: &Outcome) -> Option<Self> {
        if outcome.placed.is_empty() {
            return None;
        }
        let placed = outcome.placed.clone();
        match kind {
            TaskKind::Copy => Some(Self::Copied(placed)),
            TaskKind::Move => Some(Self::Moved(placed)),
            TaskKind::Trash => Some(Self::Trashed(placed)),
            TaskKind::Restore => Some(Self::Restored(placed)),
            TaskKind::Delete | TaskKind::Compress | TaskKind::Extract | TaskKind::Convert => None,
        }
    }

    #[must_use]
    pub fn reversal(&self) -> Task {
        match self {
            Self::Moved(placed) => Task::Relocate {
                placements: placed.iter().rev().map(Placement::reversed).collect(),
            },
            Self::Copied(placed) | Self::Restored(placed) => Task::Trash {
                items: placed
                    .iter()
                    .rev()
                    .map(|placed| placed.to.clone())
                    .collect(),
            },
            Self::Trashed(placed) => Task::Restore {
                items: placed.iter().map(|placed| placed.to.clone()).collect(),
            },
            Self::Created(items) => Task::Trash {
                items: items.iter().rev().cloned().collect(),
            },
        }
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&Location) -> bool) {
        match self {
            Self::Moved(placed)
            | Self::Copied(placed)
            | Self::Trashed(placed)
            | Self::Restored(placed) => placed.retain(|placed| keep(&placed.to)),
            Self::Created(items) => items.retain(|at| keep(at)),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Moved(placed)
            | Self::Copied(placed)
            | Self::Trashed(placed)
            | Self::Restored(placed) => placed.is_empty(),
            Self::Created(items) => items.is_empty(),
        }
    }
}
