const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeTime {
    Minutes(u64),
    Hours(u64),
    Days(u64),
}

#[must_use]
pub fn time_ago(timestamp_millis: u64) -> Option<RelativeTime> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    relative_time(now.saturating_sub(timestamp_millis / 1_000))
}

#[must_use]
pub fn relative_time(seconds: u64) -> Option<RelativeTime> {
    match seconds {
        0..MINUTE => None,
        MINUTE..HOUR => Some(RelativeTime::Minutes(seconds / MINUTE)),
        HOUR..DAY => Some(RelativeTime::Hours(seconds / HOUR)),
        _ => Some(RelativeTime::Days(seconds / DAY)),
    }
}
