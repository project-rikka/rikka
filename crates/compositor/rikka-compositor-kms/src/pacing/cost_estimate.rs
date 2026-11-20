use std::time::Duration;

use rikka_compositor_reactor::Timestamp;

const MEAN_TIME_CONSTANT: Duration = Duration::from_millis(500);

const DEVIATION_TIME_CONSTANT: Duration = Duration::from_secs(6);

const DEVIATION_FACTOR: u32 = 2;

const FIRST_SAMPLE_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CostEstimate {
    mean: Duration,
    spread: Duration,
    last: Option<Timestamp>,
}

impl CostEstimate {
    pub(crate) fn add(&mut self, cost: Duration, at: Timestamp) {
        let since = self
            .last
            .map_or(FIRST_SAMPLE_INTERVAL, |last| at.saturating_since(last));
        self.last = Some(at);
        let above = cost.saturating_sub(self.mean);
        let spread = mix(
            above,
            self.spread,
            ratio(since, DEVIATION_TIME_CONSTANT, 0.001, 0.1),
        );
        self.spread = spread.max(above);
        self.mean = mix(cost, self.mean, ratio(since, MEAN_TIME_CONSTANT, 0.01, 1.0));
    }

    pub(crate) fn estimate(&self) -> Option<Duration> {
        self.last
            .map(|_| self.mean + self.spread * DEVIATION_FACTOR)
    }
}

fn ratio(since: Duration, constant: Duration, least: f64, most: f64) -> f64 {
    (since.as_secs_f64() / constant.as_secs_f64()).clamp(least, most)
}

fn mix(new: Duration, old: Duration, ratio: f64) -> Duration {
    Duration::from_secs_f64(new.as_secs_f64() * ratio + old.as_secs_f64() * (1.0 - ratio))
}
