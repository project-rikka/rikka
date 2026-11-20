use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

use crate::sources::{Sources, TimerFn};

const COMPACT_AFTER: usize = 64;

pub(crate) struct Timers {
    due: BinaryHeap<Reverse<(Instant, u64)>>,
    stale: usize,
}

impl Timers {
    pub(crate) fn new() -> Self {
        Self {
            due: BinaryHeap::new(),
            stale: 0,
        }
    }

    pub(crate) fn schedule(&mut self, token: u64, at: Instant) {
        self.due.push(Reverse((at, token)));
    }

    pub(crate) fn cancelled(&mut self) {
        self.stale += 1;
    }

    pub(crate) fn earliest<S>(&mut self, sources: &Sources<S>) -> Option<Instant> {
        self.compact(sources);
        loop {
            let &Reverse((at, token)) = self.due.peek()?;
            if sources.timer_at(token) == Some(at) {
                return Some(at);
            }
            self.discard();
        }
    }

    pub(crate) fn pop_due<S>(
        &mut self,
        sources: &mut Sources<S>,
        now: Instant,
    ) -> Option<TimerFn<S>> {
        loop {
            let &Reverse((at, token)) = self.due.peek()?;
            if sources.timer_at(token) != Some(at) {
                self.discard();
                continue;
            }
            if at > now {
                return None;
            }
            self.due.pop();
            return sources.take_timer(token);
        }
    }

    fn discard(&mut self) {
        self.due.pop();
        self.stale = self.stale.saturating_sub(1);
    }

    fn compact<S>(&mut self, sources: &Sources<S>) {
        if self.stale < COMPACT_AFTER || self.stale * 2 < self.due.len() {
            return;
        }
        self.due
            .retain(|Reverse((at, token))| sources.timer_at(*token) == Some(*at));
        self.stale = 0;
    }
}
