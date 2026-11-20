use std::time::Duration;

pub(crate) fn frames_to_duration(frames: u64, rate: u32) -> Duration {
    if rate == 0 {
        return Duration::ZERO;
    }
    Duration::from_secs_f64(frames as f64 / rate as f64)
}

pub(crate) fn duration_to_frames(duration: Duration, rate: u32) -> u64 {
    (duration.as_secs_f64() * rate as f64).round() as u64
}
