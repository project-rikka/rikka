use std::sync::Arc;
use std::time::Duration;

use rikka_compositor_reactor::Timestamp;

use crate::seq_cell::SeqCell;

const PHASE_VALIDITY: Duration = Duration::from_secs(1);

const VRR: u64 = 1 << 0;
const PHASED: u64 = 1 << 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub refresh: Duration,
    pub vrr: bool,
    pub last_vblank: Option<Timestamp>,
    pub sequence: u64,
    pub lead: Duration,
}

#[derive(Debug, Clone)]
pub struct VblankTimeline(Arc<SeqCell<5>>);

impl Timing {
    pub fn phase(&self, around: Timestamp) -> Option<Timestamp> {
        self.last_vblank
            .filter(|vblank| around.saturating_since(*vblank) <= PHASE_VALIDITY)
    }

    pub fn next_vblank(&self, at_or_after: Timestamp) -> Option<Timestamp> {
        let vblank = self.phase(at_or_after)?;
        vblank_from(
            vblank.as_monotonic(),
            self.refresh,
            at_or_after.as_monotonic(),
        )
        .map(Timestamp::from_monotonic)
    }

    pub fn next_deadline(&self, now: Timestamp) -> Option<(Timestamp, Timestamp)> {
        let vblank = self.next_vblank(now + self.lead)?;
        Some((vblank, vblank - self.lead))
    }

    fn words(&self) -> [u64; 5] {
        let flags = match (self.vrr, self.last_vblank.is_some()) {
            (true, true) => VRR | PHASED,
            (true, false) => VRR,
            (false, true) => PHASED,
            (false, false) => 0,
        };
        [
            nanos(self.refresh),
            nanos(self.lead),
            self.last_vblank
                .map_or(0, |vblank| nanos(vblank.as_monotonic())),
            self.sequence,
            flags,
        ]
    }

    fn from_words([refresh, lead, vblank, sequence, flags]: [u64; 5]) -> Self {
        Self {
            refresh: Duration::from_nanos(refresh),
            vrr: flags & VRR != 0,
            last_vblank: (flags & PHASED != 0)
                .then(|| Timestamp::from_monotonic(Duration::from_nanos(vblank))),
            sequence,
            lead: Duration::from_nanos(lead),
        }
    }
}

pub(crate) fn nanos(span: Duration) -> u64 {
    span.as_secs()
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::from(span.subsec_nanos()))
}

pub(crate) fn vblank_from(
    phase: Duration,
    refresh: Duration,
    at_or_after: Duration,
) -> Option<Duration> {
    let cycle = nanos(refresh).max(1);
    let periods = nanos(at_or_after.saturating_sub(phase)).div_ceil(cycle);
    let ahead = periods.checked_mul(cycle)?;
    Some(phase + Duration::from_nanos(ahead))
}

impl VblankTimeline {
    pub(crate) fn new(timing: Timing) -> Self {
        Self(Arc::new(SeqCell::new(timing.words())))
    }

    pub fn timing(&self) -> Timing {
        Timing::from_words(self.0.load())
    }

    pub(crate) fn publish(&self, timing: Timing) {
        self.0.store(timing.words());
    }
}
