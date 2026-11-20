use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static PUBLISHED: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefreshRate(NonZeroU32);

impl RefreshRate {
    pub const SIXTY_HERTZ: RefreshRate = RefreshRate(NonZeroU32::new(60_000).unwrap());

    #[must_use]
    pub const fn from_millihertz(millihertz: u32) -> Option<RefreshRate> {
        match NonZeroU32::new(millihertz) {
            Some(millihertz) => Some(RefreshRate(millihertz)),
            None => None,
        }
    }

    #[must_use]
    pub const fn millihertz(self) -> u32 {
        self.0.get()
    }

    #[must_use]
    pub const fn period(self) -> Duration {
        Duration::from_nanos(1_000_000_000_000 / self.0.get() as u64)
    }

    pub fn publish(self) {
        PUBLISHED.store(self.millihertz(), Ordering::Relaxed);
    }

    #[must_use]
    pub fn published() -> Option<RefreshRate> {
        RefreshRate::from_millihertz(PUBLISHED.load(Ordering::Relaxed))
    }

    #[must_use]
    pub fn published_period() -> Duration {
        RefreshRate::published()
            .unwrap_or(RefreshRate::SIXTY_HERTZ)
            .period()
    }
}
