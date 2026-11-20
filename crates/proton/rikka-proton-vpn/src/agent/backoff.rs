use std::time::Duration;

use rikka_proton_api::delay;

const INITIAL_DELAY: Duration = Duration::from_millis(250);
const MAX_DELAY: Duration = Duration::from_secs(60);
const GROWTH_FACTOR: u32 = 4;
const JITTER_FRACTION: f64 = 0.2;

pub(crate) struct Backoff {
    next: Duration,
}

impl Backoff {
    pub(crate) fn new() -> Backoff {
        Backoff {
            next: INITIAL_DELAY,
        }
    }

    pub(crate) fn next_delay(&mut self, connected_for: Duration) -> Duration {
        let wait = self
            .next
            .min(MAX_DELAY.saturating_sub(connected_for))
            .max(INITIAL_DELAY);
        let wait = delay::stretched(wait, JITTER_FRACTION);
        self.next = (wait * GROWTH_FACTOR).min(MAX_DELAY);
        wait
    }
}
