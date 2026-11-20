use std::time::{Duration, Instant};

const FREE_ATTEMPTS: u32 = 3;
const LONGEST_WAIT: Duration = Duration::from_secs(300);

#[derive(Debug, Default)]
pub struct Throttle {
    failures: u32,
    next: Option<Instant>,
}

impl Throttle {
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.next
            .and_then(|next| next.checked_duration_since(now))
            .filter(|left| !left.is_zero())
    }

    pub fn failed(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        let Some(over) = self.failures.checked_sub(FREE_ATTEMPTS) else {
            return;
        };
        let wait = Duration::from_secs(1_u64 << over.min(16)).min(LONGEST_WAIT);
        self.next = Some(now + wait);
    }

    pub fn succeeded(&mut self) {
        *self = Throttle::default();
    }
}
