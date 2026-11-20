use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    at: Duration,
    since: Option<Instant>,
    length: Duration,
}

impl Clock {
    #[must_use]
    pub fn new(length: Duration) -> Clock {
        Clock {
            at: Duration::ZERO,
            since: None,
            length,
        }
    }

    pub fn run(&mut self, at: Duration, now: Instant) {
        self.at = at;
        self.since = Some(now);
    }

    pub fn stop(&mut self, at: Duration) {
        self.at = at;
        self.since = None;
    }

    pub fn pause(&mut self, now: Instant) {
        self.at = self.position(now);
        self.since = None;
    }

    #[must_use]
    pub fn running(&self) -> bool {
        self.since.is_some()
    }

    #[must_use]
    pub fn length(&self) -> Duration {
        self.length
    }

    pub fn set_length(&mut self, length: Duration) {
        self.length = length;
    }

    #[must_use]
    pub fn position(&self, now: Instant) -> Duration {
        let travelled = self
            .since
            .map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
        let at = self.at + travelled;
        match self.length.is_zero() {
            true => at,
            false => at.min(self.length),
        }
    }

    #[must_use]
    pub fn progress(&self, now: Instant) -> f32 {
        match self.length.is_zero() {
            true => 0.0,
            false => (self.position(now).as_secs_f64() / self.length.as_secs_f64()) as f32,
        }
    }
}
