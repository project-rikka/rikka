use std::time::Duration;

pub(crate) use rikka_settings::session::notifications::*;

#[must_use]
pub(crate) fn linger(seconds: i64) -> Duration {
    Duration::from_secs(seconds.clamp(*TIMEOUTS.start(), *TIMEOUTS.end()) as u64)
}

#[must_use]
pub(crate) fn shown(count: i64) -> usize {
    count.clamp(*VISIBLES.start(), *VISIBLES.end()) as usize
}
