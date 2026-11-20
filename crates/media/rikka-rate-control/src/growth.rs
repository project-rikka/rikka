use std::time::Duration;

pub(crate) const LONGEST_STEP: Duration = Duration::from_secs(1);
const GROWTH_PER_SECOND: f64 = 1.08;
const ACKED_HEADROOM: f64 = 1.5;
const ACKED_SLACK_BPS: f64 = 10_000.0;

pub(crate) fn multiplied(rate: f64, step: f64) -> f64 {
    rate * GROWTH_PER_SECOND.powf(step)
}

pub(crate) fn within_acked(rate: f64, increased: f64, acked: Option<f64>) -> f64 {
    match acked {
        Some(acked) => increased.min(rate.max(ACKED_HEADROOM * acked + ACKED_SLACK_BPS)),
        None => increased,
    }
}
