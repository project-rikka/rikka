use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct Wake {
    scheduled: Instant,
    woke: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lateness {
    timer_delay: Duration,
    dispatched: Duration,
}

impl Wake {
    pub fn new(scheduled: Instant, woke: Instant) -> Self {
        Self { scheduled, woke }
    }

    pub fn reached(self, now: Instant) -> Lateness {
        Lateness {
            timer_delay: self.woke.saturating_duration_since(self.scheduled),
            dispatched: now.saturating_duration_since(self.woke),
        }
    }
}

impl Lateness {
    pub fn new(timer_delay: Duration, dispatched: Duration) -> Self {
        Self {
            timer_delay,
            dispatched,
        }
    }

    pub fn timer_delay(self) -> Duration {
        self.timer_delay
    }

    pub fn dispatched(self) -> Duration {
        self.dispatched
    }

    pub fn total(self) -> Duration {
        self.timer_delay + self.dispatched
    }
}
