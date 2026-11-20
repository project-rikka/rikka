use std::ops::RangeInclusive;
use std::sync::Arc;

use rikka_gpu::Compute;

use crate::error::Error;
use crate::layout::Layout;
use crate::scene_source::SceneSource;

#[derive(Default)]
pub(super) struct BakeState {
    pub(super) compute: Option<Compute>,
    pub(super) source: Option<Arc<dyn SceneSource>>,
    pub(super) pending: Option<Option<u32>>,
    pub(super) paused: bool,
    pub(super) target_step: u32,
    pub(super) playhead: u32,
    pub(super) full: bool,
    pub(super) frontier: Option<u32>,
    pub(super) busy: bool,
    pub(super) layout: Option<Arc<Layout>>,
    pub(super) epoch: u64,
    pub(super) coverage: Vec<RangeInclusive<u32>>,
    pub(super) halted: bool,
    pub(super) error: Option<Error>,
    pub(super) quit: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub cached: Vec<RangeInclusive<u32>>,
    pub target_step: u32,
    pub working: bool,
}

#[derive(Clone, Debug)]
pub(super) struct BakeStatus {
    pub(super) layout: Option<Arc<Layout>>,
    pub(super) epoch: u64,
    pub(super) showable: bool,
}

impl BakeState {
    pub(super) fn set_source(&mut self, source: Arc<dyn SceneSource>, since: Option<u32>) {
        self.source = Some(source);
        let merged = match (self.pending.flatten(), since) {
            (Some(pending), Some(since)) => Some(pending.min(since)),
            (pending, since) => pending.or(since),
        };
        self.pending = Some(merged);
    }

    pub(super) fn set_playhead(&mut self, step: u32) -> bool {
        let moved = self.playhead != step;
        if moved {
            self.playhead = step;
            self.full = false;
        }
        moved
    }

    pub(super) fn wanted(&self) -> Option<u32> {
        if self.paused || self.halted || self.full || self.playhead > self.target_step {
            return None;
        }
        let step = match self.covering(self.playhead) {
            Some(range) => range.end().checked_add(1)?,
            None => self.playhead,
        };
        (step <= self.target_step).then_some(step)
    }

    fn covering(&self, step: u32) -> Option<&RangeInclusive<u32>> {
        self.coverage.iter().find(|range| range.contains(&step))
    }

    pub(super) fn progress(&self) -> Progress {
        Progress {
            cached: self.coverage.clone(),
            target_step: self.target_step,
            working: self.busy
                || (self.compute.is_some() && self.pending.is_some())
                || (self.layout.is_some() && self.wanted().is_some()),
        }
    }

    pub(super) fn status(&self, step: u32) -> BakeStatus {
        let settled = self
            .frontier
            .is_some_and(|frontier| step < frontier || (step == frontier && !self.busy));
        BakeStatus {
            layout: self.layout.clone(),
            epoch: self.epoch,
            showable: settled && self.covering(step).is_some(),
        }
    }
}
