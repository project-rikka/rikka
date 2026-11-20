use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moment {
    pub unix_ms: i64,
    pub instant: Instant,
}

impl Moment {
    #[must_use]
    pub fn now() -> Moment {
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
            });
        Moment {
            unix_ms,
            instant: Instant::now(),
        }
    }
}
