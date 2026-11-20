use std::time::Duration;

use crate::assignment::{Assignment, Blocked, Reason};

const PROMOTION_DELAY: Duration = Duration::from_millis(100);

const PROMOTION_BUFFERS: u64 = 2;

#[derive(Debug, Clone, Copy)]
struct Candidate<S> {
    surface: S,
    since: Duration,
    generation: u64,
}

#[derive(Debug, Clone)]
pub struct Promotion<S> {
    placed: Option<S>,
    waiting: Option<Candidate<S>>,
}

impl<S> Default for Promotion<S> {
    fn default() -> Self {
        Self {
            placed: None,
            waiting: None,
        }
    }
}

impl<S: Copy + Eq> Promotion<S> {
    pub fn update(
        &mut self,
        assignment: Assignment<S>,
        now: Duration,
        generation: Option<u64>,
    ) -> Assignment<S> {
        let Some(layout) = assignment.layout() else {
            self.reset();
            return assignment;
        };
        let Some(generation) = generation else {
            self.reset();
            return Assignment::Composited(None);
        };
        let surface = layout.surface;
        if self.placed == Some(surface) {
            return assignment;
        }
        self.placed = None;
        let candidate = match self.waiting.filter(|waiting| waiting.surface == surface) {
            Some(waiting) => waiting,
            None => Candidate {
                surface,
                since: now,
                generation,
            },
        };
        let ready = now.saturating_sub(candidate.since) >= PROMOTION_DELAY
            && generation.saturating_sub(candidate.generation) >= PROMOTION_BUFFERS;
        if ready {
            self.waiting = None;
            self.placed = Some(surface);
            return assignment;
        }
        self.waiting = Some(candidate);
        Assignment::Composited(Some(Blocked {
            surface,
            reason: Reason::Pending,
        }))
    }

    pub fn reset(&mut self) {
        self.placed = None;
        self.waiting = None;
    }
}
