use std::time::Duration;

use rikka_compositor_reactor::Timestamp;

pub(crate) fn monotonic() -> Duration {
    Timestamp::now().as_monotonic()
}
