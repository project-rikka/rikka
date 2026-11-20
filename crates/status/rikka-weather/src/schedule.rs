use std::time::{Duration, SystemTime};

pub(crate) const FORECAST_LIFE: Duration = Duration::from_secs(15 * 60);
pub(crate) const AIR_LIFE: Duration = Duration::from_secs(60 * 60);

const BACKOFF: [Duration; 6] = [
    Duration::from_secs(30),
    Duration::from_secs(60),
    Duration::from_secs(120),
    Duration::from_secs(300),
    Duration::from_secs(600),
    Duration::from_secs(900),
];

#[derive(Debug, Default)]
pub(crate) struct Schedule {
    failures: usize,
    next: Option<SystemTime>,
}

impl Schedule {
    pub(crate) fn fetched(&mut self, at: SystemTime) {
        self.failures = 0;
        self.next = Some(at + FORECAST_LIFE);
    }

    pub(crate) fn failed(&mut self, now: SystemTime) {
        self.next = Some(now + BACKOFF[self.failures.min(BACKOFF.len() - 1)]);
        self.failures += 1;
    }

    pub(crate) fn soon(&mut self) {
        self.failures = 0;
        self.next = None;
    }

    pub(crate) fn due(&self, now: SystemTime) -> bool {
        self.next.is_none_or(|next| now >= next)
    }

    pub(crate) fn next(&self) -> Option<SystemTime> {
        self.next
    }

    pub(crate) fn failing(&self) -> bool {
        self.failures > 0
    }
}
