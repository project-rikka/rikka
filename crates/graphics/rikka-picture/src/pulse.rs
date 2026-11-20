use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick {
    pub at: Instant,
    pub period: Duration,
}

impl Tick {
    #[must_use]
    pub fn next(self) -> Instant {
        self.at + self.period
    }
}

#[derive(Clone)]
pub struct Pulse(Arc<dyn Fn(Tick) + Send + Sync>);

impl Pulse {
    pub fn new(beat: impl Fn(Tick) + Send + Sync + 'static) -> Pulse {
        Pulse(Arc::new(beat))
    }

    pub fn beat(&self, tick: Tick) {
        (self.0)(tick);
    }

    pub fn join(into: &mut Vec<Pulse>, pulse: Pulse) {
        if !into.iter().any(|other| Arc::ptr_eq(&other.0, &pulse.0)) {
            into.push(pulse);
        }
    }
}

impl std::fmt::Debug for Pulse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Pulse").finish_non_exhaustive()
    }
}
