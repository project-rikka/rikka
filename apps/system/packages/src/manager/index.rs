use std::sync::Arc;

use nixpkgs_search::{Catalog, Stage};

use crate::error::Error;

const PROGRESS_STEPS: u64 = 200;
const UNSIZED_STEP: u64 = 1 << 20;

#[derive(Debug)]
pub(super) enum Index {
    Building(Stage),
    Ready(Arc<Catalog>),
    Failed(Error),
}

impl Default for Index {
    fn default() -> Self {
        Self::Building(Stage::Locating)
    }
}

impl Index {
    pub(super) fn catalog(&self) -> Option<&Arc<Catalog>> {
        match self {
            Self::Ready(catalog) => Some(catalog),
            _ => None,
        }
    }

    pub(super) fn is_finished(&self) -> bool {
        !matches!(self, Self::Building(_))
    }

    pub(super) fn advance(&mut self, stage: Stage) -> bool {
        let Self::Building(current) = self else {
            return false;
        };
        if step(*current) == step(stage) {
            return false;
        }
        *current = stage;
        true
    }
}

fn step(stage: Stage) -> (u8, u64) {
    match stage {
        Stage::Locating => (0, 0),
        Stage::Downloading {
            received,
            total: Some(total),
        } if total > 0 => (1, received.saturating_mul(PROGRESS_STEPS) / total),
        Stage::Downloading { received, .. } => (2, received / UNSIZED_STEP),
        Stage::Indexing => (3, 0),
    }
}
