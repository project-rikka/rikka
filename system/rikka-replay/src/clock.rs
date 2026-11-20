use std::time::Duration;

use rustix::time::{ClockId, clock_gettime};

pub(crate) fn now() -> Duration {
    let now = clock_gettime(ClockId::Monotonic);
    Duration::new(
        now.tv_sec.max(0) as u64,
        now.tv_nsec.clamp(0, 999_999_999) as u32,
    )
}

pub(crate) fn micros(at: Duration) -> i64 {
    at.as_micros().min(i64::MAX as u128) as i64
}

pub(crate) fn now_us() -> i64 {
    micros(now())
}
