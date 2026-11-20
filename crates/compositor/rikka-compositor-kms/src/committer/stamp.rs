use std::time::Duration;

use rikka_compositor_reactor::Timestamp;

const STALE_AFTER_REFRESHES: u32 = 2;

pub(super) fn vblank_at(
    reported: Option<Timestamp>,
    now: Timestamp,
    refresh: Duration,
) -> Timestamp {
    match reported {
        Some(at) if at <= now && now.saturating_since(at) <= refresh * STALE_AFTER_REFRESHES => at,
        _ => now,
    }
}
