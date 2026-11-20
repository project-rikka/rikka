use std::time::{Duration, Instant};

use rikka_types::{Modifiers, Point};

use crate::input::{DRAG_SLOP, ScrollPhase, ScrollSource};

const WHEEL_TRANSACTION_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Default)]
pub(crate) struct Latch {
    current: Option<LatchedTarget>,
}

#[derive(Debug)]
struct LatchedTarget {
    target: u64,
    at: Point,
    modifiers: Modifiers,
    last: Instant,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollStep {
    pub(crate) source: ScrollSource,
    pub(crate) at: Point,
    pub(crate) modifiers: Modifiers,
    pub(crate) now: Instant,
}

impl Latch {
    pub(crate) fn target(&mut self, step: ScrollStep) -> Option<u64> {
        let latched = self.current.as_ref()?;
        let continues = step.source.phase() != Some(ScrollPhase::Began)
            && latched.modifiers == step.modifiers
            && step.now.saturating_duration_since(latched.last) <= WHEEL_TRANSACTION_TIMEOUT
            && step.at.distance(latched.at) <= DRAG_SLOP;
        if !continues {
            self.current = None;
        }
        self.current.as_ref().map(|latched| latched.target)
    }

    pub(crate) fn update(&mut self, step: ScrollStep, handler: Option<u64>) {
        let target = match (step.source.phase(), handler) {
            (Some(ScrollPhase::Ended), _) | (_, None) => {
                self.current = None;
                return;
            }
            (_, Some(target)) => target,
        };
        match self.current.as_mut() {
            Some(latched) if latched.target == target => latched.last = step.now,
            _ => {
                self.current = Some(LatchedTarget {
                    target,
                    at: step.at,
                    modifiers: step.modifiers,
                    last: step.now,
                });
            }
        }
    }
}
