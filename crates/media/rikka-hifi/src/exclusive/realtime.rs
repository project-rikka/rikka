use std::time::Duration;

use rikka_rtkit::{Grant, Grantor};

const PRIORITY: u32 = 15;
const BUDGET: Duration = Duration::from_millis(200);

pub(crate) fn request_realtime() {
    match rikka_rtkit::make_thread_realtime(PRIORITY, BUDGET) {
        Ok(Grant {
            level,
            grantor: Grantor::Kernel,
        }) => tracing::debug!("ALSA writer runs at SCHED_FIFO {level}"),
        Ok(Grant {
            level,
            grantor: Grantor::RealtimeKit,
        }) => tracing::debug!("RealtimeKit set ALSA writer to SCHED_FIFO {level}"),
        Err(error) => tracing::debug!("ALSA writer keeps normal priority: {error}"),
    }
}
