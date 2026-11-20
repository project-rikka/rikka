use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct FrameTimes {
    pub frames: usize,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub jank_percent: f64,
}

const JANK_FACTOR: f64 = 1.5;

impl FrameTimes {
    #[must_use]
    pub fn from_stamps(stamps: &[u64], refresh: Duration) -> FrameTimes {
        let mut intervals: Vec<f64> = stamps
            .windows(2)
            .map(|pair| (pair[1] - pair[0]) as f64 / 1e6)
            .collect();
        intervals.sort_by(f64::total_cmp);
        let budget = refresh.as_secs_f64() * 1e3 * JANK_FACTOR;
        let janky = intervals
            .iter()
            .filter(|interval| **interval > budget)
            .count();
        FrameTimes {
            frames: stamps.len(),
            p50_ms: percentile(&intervals, 50.0),
            p95_ms: percentile(&intervals, 95.0),
            p99_ms: percentile(&intervals, 99.0),
            max_ms: intervals.last().copied().unwrap_or_default(),
            jank_percent: match intervals.len() {
                0 => 0.0,
                count => janky as f64 * 100.0 / count as f64,
            },
        }
    }
}

#[must_use]
pub(crate) fn percentile(sorted: &[f64], percent: f64) -> f64 {
    match sorted.len() {
        0 => 0.0,
        count => sorted[((count - 1) as f64 * percent / 100.0).round() as usize],
    }
}

#[must_use]
pub(crate) fn median(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut values: Vec<f64> = values.into_iter().collect();
    values.sort_by(f64::total_cmp);
    percentile(&values, 50.0)
}

#[must_use]
pub(crate) fn spread(values: impl IntoIterator<Item = f64>) -> (f64, f64) {
    values
        .into_iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
            (low.min(value), high.max(value))
        })
}
