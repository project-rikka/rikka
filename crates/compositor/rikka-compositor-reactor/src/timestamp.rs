use std::ops::{Add, Sub};
use std::time::{Duration, Instant};

use rustix::time::{ClockId, clock_gettime};

const NANOS_PER_SECOND: u64 = 1_000_000_000;
const NANOS_PER_MICRO: u64 = 1_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    nanos: u64,
}

impl Timestamp {
    pub fn now() -> Self {
        let now = clock_gettime(ClockId::Monotonic);
        let seconds = u64::try_from(now.tv_sec).unwrap_or_default();
        let nanos = u64::try_from(now.tv_nsec).unwrap_or_default();
        Self {
            nanos: seconds
                .saturating_mul(NANOS_PER_SECOND)
                .saturating_add(nanos),
        }
    }

    pub const fn from_monotonic(since_boot: Duration) -> Self {
        let nanos = since_boot.as_nanos();
        Self {
            nanos: if nanos > u64::MAX as u128 {
                u64::MAX
            } else {
                nanos as u64
            },
        }
    }

    pub const fn from_micros(since_boot: u64) -> Self {
        Self {
            nanos: since_boot.saturating_mul(NANOS_PER_MICRO),
        }
    }

    pub fn from_instant(at: Instant) -> Self {
        let now = Self::now();
        let instant = Instant::now();
        match at >= instant {
            true => now + (at - instant),
            false => now.saturating_sub(instant - at),
        }
    }

    pub const fn as_monotonic(self) -> Duration {
        Duration::from_nanos(self.nanos)
    }

    pub fn saturating_since(self, earlier: Self) -> Duration {
        Duration::from_nanos(self.nanos.saturating_sub(earlier.nanos))
    }

    pub fn saturating_sub(self, span: Duration) -> Self {
        Self {
            nanos: self.nanos.saturating_sub(Self::from_monotonic(span).nanos),
        }
    }

    pub fn to_instant(self) -> Instant {
        let now = Self::now();
        let instant = Instant::now();
        match self >= now {
            true => instant + self.saturating_since(now),
            false => instant
                .checked_sub(now.saturating_since(self))
                .unwrap_or(instant),
        }
    }
}

impl Add<Duration> for Timestamp {
    type Output = Self;

    fn add(self, span: Duration) -> Self {
        Self {
            nanos: self.nanos.saturating_add(Self::from_monotonic(span).nanos),
        }
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Self;

    fn sub(self, span: Duration) -> Self {
        self.saturating_sub(span)
    }
}
