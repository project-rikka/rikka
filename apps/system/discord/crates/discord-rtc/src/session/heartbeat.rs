use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::time::Instant;

#[derive(Debug)]
pub(crate) struct Heartbeat {
    interval: Duration,
    due: Instant,
    pending: Option<(u64, Instant)>,
}

impl Heartbeat {
    pub(crate) fn new(interval: Duration, now: Instant) -> Heartbeat {
        Heartbeat {
            interval,
            due: now + interval,
            pending: None,
        }
    }

    pub(crate) fn due(&self) -> Instant {
        self.due
    }

    pub(crate) fn beat(&mut self, now: Instant) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as u64);
        self.pending = Some((nonce, now));
        self.due = now + self.interval;
        Some(nonce)
    }

    pub(crate) fn acknowledged(&mut self, nonce: u64, now: Instant) -> Option<Duration> {
        match self.pending {
            Some((sent, at)) if sent == nonce => {
                self.pending = None;
                Some(now.saturating_duration_since(at))
            }
            _ => None,
        }
    }
}
