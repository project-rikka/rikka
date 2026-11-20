use rikka_rtkit::{Grant, Grantor};

const NICE: i32 = -10;

pub(crate) fn raise_frame_loop() {
    match rikka_rtkit::make_thread_high_priority(NICE) {
        Ok(Grant {
            level,
            grantor: Grantor::Kernel,
        }) => tracing::info!("frame loop niceness {level}"),
        Ok(Grant {
            level,
            grantor: Grantor::RealtimeKit,
        }) => tracing::info!("RealtimeKit set frame loop niceness {level}"),
        Err(error) => tracing::info!("frame loop runs at normal priority: {error}"),
    }
}
