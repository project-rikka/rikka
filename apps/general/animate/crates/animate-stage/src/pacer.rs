use std::time::{Duration, Instant};

const FRAMES_PER_SECOND: u64 = 60;
const INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / FRAMES_PER_SECOND);
const SLACK: Duration = Duration::from_millis(2);

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Pacer {
    due: Option<Instant>,
}

impl Pacer {
    pub(super) fn is_due(&self, now: Instant) -> bool {
        self.due.is_none_or(|due| now + SLACK >= due)
    }

    pub(super) fn due(&self, now: Instant) -> Instant {
        self.due.map_or(now, |due| due.max(now))
    }

    pub(super) fn drew(&mut self, now: Instant) {
        let next = match self.due {
            Some(due) if now < due + INTERVAL => due + INTERVAL,
            _ => now + INTERVAL,
        };
        self.due = Some(next);
    }
}
