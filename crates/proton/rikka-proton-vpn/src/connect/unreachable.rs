use std::time::{Duration, Instant};

use rikka_proton_api::delay;

const MIN_FALLBACK_INTERVAL: Duration = Duration::from_secs(30);
const MAX_FALLBACK_INTERVAL: Duration = Duration::from_secs(15 * 60);
const JITTER_FRACTION: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnreachableAction {
    SilentReconnect,
    Fallback,
    Error,
}

#[derive(Debug, Default)]
pub(crate) struct UnreachableTracker {
    connected: bool,
    fallbacks: u32,
    last_fallback: Option<Instant>,
    last_action: Option<UnreachableAction>,
}

impl UnreachableTracker {
    pub(crate) fn record_unreachable(&mut self) -> UnreachableAction {
        let was_connected = self.connected;
        self.connected = false;
        let now = Instant::now();
        let last_fallback_at = *self.last_fallback.get_or_insert(now);

        let action = if was_connected {
            UnreachableAction::SilentReconnect
        } else if now.duration_since(last_fallback_at) >= self.interval() {
            UnreachableAction::Fallback
        } else {
            UnreachableAction::Error
        };
        self.last_action = Some(action);
        action
    }

    pub(crate) fn record_fallback(&mut self) {
        self.fallbacks += 1;
        self.last_fallback = Some(Instant::now());
    }

    pub(crate) fn is_silent(&self) -> bool {
        self.last_action == Some(UnreachableAction::SilentReconnect)
    }

    pub(crate) fn reset(&mut self, connected: bool) {
        *self = UnreachableTracker {
            connected,
            ..UnreachableTracker::default()
        };
    }

    fn interval(&self) -> Duration {
        let steps = 1 + self.fallbacks;
        let base = MIN_FALLBACK_INTERVAL
            .saturating_mul(steps.saturating_mul(steps))
            .min(MAX_FALLBACK_INTERVAL);
        delay::stretched(base, JITTER_FRACTION)
    }
}
