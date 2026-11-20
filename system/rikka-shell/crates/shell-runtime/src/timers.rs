use std::time::{Duration, Instant};

use super::surfaces::SurfaceId;

const UNTIL_WOKEN: Duration = Duration::from_millis(0x7fff_ffff);

#[derive(Debug)]
pub(super) enum Alarm {
    ExitTimeout { desc: SurfaceId, leave: u64 },
}

#[derive(Debug, Default)]
pub(super) struct Timers {
    set: Vec<(Instant, Alarm)>,
}

impl Timers {
    pub(crate) fn at(&mut self, due: Instant, alarm: Alarm) {
        self.set.push((due, alarm));
    }

    #[must_use]
    pub(crate) fn next(&self) -> Option<Instant> {
        self.set.iter().map(|(due, _)| *due).min()
    }

    pub(super) fn take_due(&mut self, now: Instant) -> Vec<Alarm> {
        if self.next().is_none_or(|earliest| earliest > now) {
            return Vec::new();
        }
        let mut due: Vec<_> = self.set.extract_if(.., |(at, _)| *at <= now).collect();
        due.sort_by_key(|(at, _)| *at);
        due.into_iter().map(|(_, alarm)| alarm).collect()
    }
}

#[must_use]
pub(super) fn wait_until(deadline: Option<Instant>, now: Instant) -> Duration {
    let Some(deadline) = deadline else {
        return UNTIL_WOKEN;
    };
    let left = deadline.saturating_duration_since(now);
    Duration::from_millis(u64::try_from(left.as_micros().div_ceil(1_000)).unwrap_or(u64::MAX))
        .min(UNTIL_WOKEN)
}
