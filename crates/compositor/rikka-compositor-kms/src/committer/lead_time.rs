use std::time::Duration;

use rikka_compositor_reactor::Timestamp;

use crate::pacing::{CostEstimate, DecayingPeak};

const SAFETY: Duration = Duration::from_micros(500);

const UNMEASURED_COMMIT: Duration = Duration::from_micros(500);

#[derive(Debug, Clone, Default)]
pub(crate) struct LeadTime {
    commits: CostEstimate,
    lateness: DecayingPeak,
}

impl LeadTime {
    pub(crate) fn committed(&mut self, took: Duration, refresh: Duration) {
        self.commits.add(took.min(refresh / 2), Timestamp::now());
    }

    pub(crate) fn woke(&mut self, late: Duration) {
        self.lateness.add(late);
    }

    pub(crate) fn before_vblank(&self, blanking: Duration, refresh: Duration) -> Duration {
        let commit = self.commits.estimate().unwrap_or(UNMEASURED_COMMIT);
        (blanking + commit + self.lateness.get() + SAFETY).min(refresh / 2)
    }
}
