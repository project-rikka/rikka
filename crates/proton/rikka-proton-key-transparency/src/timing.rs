use std::time::Duration;

pub(crate) const MINUTE: u64 = 60;
pub(crate) const HOUR: u64 = 60 * MINUTE;
pub(crate) const DAY: u64 = 24 * HOUR;

pub(crate) const MAX_EPOCH_INTERVAL: u64 = 72 * HOUR;
pub(crate) const SELF_AUDIT_INTERVAL: u64 = 4 * HOUR;
pub(crate) const EPOCH_CERTIFICATE_LIFETIME: u64 = 90 * DAY;
pub(crate) const LATEST_EPOCH_LIFETIME: u64 = 2 * HOUR;
pub(crate) const SELF_AUDIT_MAX_ATTEMPTS: u32 = 6;

pub(crate) fn is_past_epoch_interval(time: u64, now: u64) -> bool {
    now > time.saturating_add(MAX_EPOCH_INTERVAL)
}

pub(crate) fn is_past_certificate_lifetime(time: u64, now: u64) -> bool {
    time < now.saturating_sub(EPOCH_CERTIFICATE_LIFETIME)
}

pub(crate) fn retry_delay(failed_attempts: u32) -> u64 {
    rikka_proton_api::delay::doubling(
        Duration::from_secs(MINUTE),
        failed_attempts,
        Duration::from_secs(SELF_AUDIT_INTERVAL),
    )
    .as_secs()
}
