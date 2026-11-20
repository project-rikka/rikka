use std::time::{Duration, Instant};

const IDLE_LIMIT: Duration = Duration::from_secs(5);

pub(crate) fn deadline() -> Instant {
    Instant::now() + IDLE_LIMIT
}
