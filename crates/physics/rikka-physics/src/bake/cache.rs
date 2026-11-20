use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::sync::Arc;

use crate::snapshot::{Capture, Checkpoint};

pub(super) const DEFAULT_BUDGET: usize = 2 << 30;
pub(super) const DEFAULT_CHECKPOINT_INTERVAL: u32 = 30;

pub(super) struct Cache {
    budget: usize,
    checkpoint_interval: u32,
    captures: BTreeMap<u32, Arc<Capture>>,
    checkpoints: BTreeMap<u32, Arc<Checkpoint>>,
    bytes: usize,
}

impl Cache {
    pub(super) fn new(budget: usize, checkpoint_interval: u32) -> Cache {
        Cache {
            budget,
            checkpoint_interval: checkpoint_interval.max(1),
            captures: BTreeMap::new(),
            checkpoints: BTreeMap::new(),
            bytes: 0,
        }
    }

    pub(super) fn contains(&self, step: u32) -> bool {
        self.captures.contains_key(&step)
    }

    pub(super) fn wants_checkpoint(&self, step: u32) -> bool {
        step.is_multiple_of(self.checkpoint_interval) && !self.checkpoints.contains_key(&step)
    }

    pub(super) fn insert(
        &mut self,
        step: u32,
        capture: Option<Capture>,
        checkpoint: Option<Checkpoint>,
        playhead: u32,
    ) -> bool {
        if let Some(capture) = capture {
            self.bytes += capture.bytes();
            if let Some(replaced) = self.captures.insert(step, Arc::new(capture)) {
                self.bytes -= replaced.bytes();
            }
        }
        if let Some(checkpoint) = checkpoint {
            self.bytes += checkpoint.bytes();
            if let Some(replaced) = self.checkpoints.insert(step, Arc::new(checkpoint)) {
                self.bytes -= replaced.bytes();
            }
        }
        self.trim(playhead);
        self.captures.contains_key(&step)
    }

    pub(super) fn checkpoint_before(&self, step: u32) -> Option<(u32, Arc<Checkpoint>)> {
        self.checkpoints
            .range(..=step)
            .next_back()
            .map(|(at, checkpoint)| (*at, Arc::clone(checkpoint)))
    }

    pub(super) fn capture(&self, step: u32) -> Option<Arc<Capture>> {
        self.captures.get(&step).map(Arc::clone)
    }

    pub(super) fn invalidate_from(&mut self, step: u32) {
        self.captures.retain(|&at, _| at < step);
        self.checkpoints.retain(|&at, _| at < step);
        self.recount();
    }

    pub(super) fn clear(&mut self) {
        self.captures.clear();
        self.checkpoints.clear();
        self.bytes = 0;
    }

    pub(super) fn coverage(&self) -> Vec<RangeInclusive<u32>> {
        let mut ranges: Vec<RangeInclusive<u32>> = Vec::new();
        for &step in self.captures.keys() {
            match ranges.last_mut() {
                Some(range) if *range.end() + 1 == step => *range = *range.start()..=step,
                _ => ranges.push(step..=step),
            }
        }
        ranges
    }

    fn recount(&mut self) {
        self.bytes = self
            .captures
            .values()
            .map(|capture| capture.bytes())
            .sum::<usize>()
            + self
                .checkpoints
                .values()
                .map(|checkpoint| checkpoint.bytes())
                .sum::<usize>();
    }

    fn trim(&mut self, playhead: u32) {
        while self.bytes > self.budget {
            let capture = farthest(&self.captures, playhead, Some(playhead));
            let anchor = self
                .checkpoints
                .range(..=playhead)
                .next_back()
                .map(|(at, _)| *at);
            let checkpoint = farthest(&self.checkpoints, playhead, anchor);
            match (capture, checkpoint) {
                (Some(capture), Some(checkpoint))
                    if checkpoint.abs_diff(playhead) >= capture.abs_diff(playhead) =>
                {
                    self.remove_checkpoint(checkpoint);
                }
                (Some(capture), _) => self.remove_capture(capture),
                (None, Some(checkpoint)) => self.remove_checkpoint(checkpoint),
                (None, None) => break,
            }
        }
    }

    fn remove_capture(&mut self, step: u32) {
        if let Some(removed) = self.captures.remove(&step) {
            self.bytes -= removed.bytes();
        }
    }

    fn remove_checkpoint(&mut self, step: u32) {
        if let Some(removed) = self.checkpoints.remove(&step) {
            self.bytes -= removed.bytes();
        }
    }
}

fn farthest<V>(entries: &BTreeMap<u32, V>, playhead: u32, kept: Option<u32>) -> Option<u32> {
    let mut steps = entries.keys().copied().filter(|step| Some(*step) != kept);
    let first = steps.next()?;
    let last = steps.next_back().unwrap_or(first);
    if first.abs_diff(playhead) >= last.abs_diff(playhead) {
        Some(first)
    } else {
        Some(last)
    }
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("captures", &self.captures.len())
            .field("checkpoints", &self.checkpoints.len())
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}
