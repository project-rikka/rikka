use std::time::Duration;

use rikka_rtkit::{Grant, Grantor};

const PRIORITY: u32 = 10;
const BUDGET: Duration = Duration::from_millis(200);

pub(super) fn make_realtime() {
    match rikka_rtkit::make_thread_realtime(PRIORITY, BUDGET) {
        Ok(Grant {
            level,
            grantor: Grantor::Kernel,
        }) => tracing::info!("KMS thread runs at SCHED_FIFO {level}"),
        Ok(Grant {
            level,
            grantor: Grantor::RealtimeKit,
        }) => tracing::info!("RealtimeKit set KMS thread to SCHED_FIFO {level}"),
        Err(error) => tracing::debug!("KMS thread keeps normal priority: {error}"),
    }
    if let Err(error) = rikka_rtkit::drop_sys_nice() {
        tracing::warn!("failed to drop CAP_SYS_NICE: {error}");
    }
}
