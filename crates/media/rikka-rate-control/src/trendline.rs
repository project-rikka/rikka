use std::collections::VecDeque;

use crate::inter_arrival::Delta;

const WINDOW: usize = 20;
const SMOOTHING: f64 = 0.9;
const GAIN: f64 = 4.0;
const MOST_DELTAS: usize = 60;
const FIRST_THRESHOLD_MS: f64 = 12.5;
const LOWEST_THRESHOLD_MS: f64 = 6.0;
const HIGHEST_THRESHOLD_MS: f64 = 600.0;
const RISE: f64 = 0.0087;
const FALL: f64 = 0.039;
const IGNORED_SPIKE_MS: f64 = 15.0;
const LONGEST_STEP_MS: f64 = 100.0;
const OVERUSE_MS: f64 = 10.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Usage {
    #[default]
    Normal,
    Over,
    Under,
}

#[derive(Debug)]
pub(crate) struct Trendline {
    points: VecDeque<(f64, f64)>,
    accumulated: f64,
    smoothed: f64,
    first_arrival: Option<i64>,
    deltas: usize,
    threshold: f64,
    last_threshold_at: Option<i64>,
    over_ms: Option<f64>,
    over_count: u32,
    previous_trend: f64,
    usage: Usage,
}

impl Default for Trendline {
    fn default() -> Trendline {
        Trendline {
            points: VecDeque::with_capacity(WINDOW),
            accumulated: 0.0,
            smoothed: 0.0,
            first_arrival: None,
            deltas: 0,
            threshold: FIRST_THRESHOLD_MS,
            last_threshold_at: None,
            over_ms: None,
            over_count: 0,
            previous_trend: 0.0,
            usage: Usage::Normal,
        }
    }
}

impl Trendline {
    pub(crate) fn update(&mut self, delta: Delta) -> Usage {
        let first = *self.first_arrival.get_or_insert(delta.arrival_us);
        self.deltas = (self.deltas + 1).min(MOST_DELTAS);
        self.accumulated += delta.arrival_ms - delta.send_ms;
        self.smoothed = SMOOTHING * self.smoothed + (1.0 - SMOOTHING) * self.accumulated;
        let x = (delta.arrival_us - first) as f64 / 1000.0;
        self.points.push_back((x, self.smoothed));
        if self.points.len() > WINDOW {
            self.points.pop_front();
        }
        let trend = match self.points.len() == WINDOW {
            true => slope(&self.points).unwrap_or(self.previous_trend),
            false => self.previous_trend,
        };
        self.detect(trend, delta.send_ms, delta.arrival_us);
        self.usage
    }

    fn detect(&mut self, trend: f64, send_ms: f64, now_us: i64) {
        let modified = self.deltas as f64 * trend * GAIN;
        if modified > self.threshold {
            let over = match self.over_ms {
                Some(over) => over + send_ms,
                None => send_ms / 2.0,
            };
            self.over_ms = Some(over);
            self.over_count += 1;
            if over > OVERUSE_MS && self.over_count > 1 && trend >= self.previous_trend {
                self.over_ms = Some(0.0);
                self.over_count = 0;
                self.usage = Usage::Over;
            }
        } else if modified < -self.threshold {
            self.over_ms = None;
            self.over_count = 0;
            self.usage = Usage::Under;
        } else {
            self.over_ms = None;
            self.over_count = 0;
            self.usage = Usage::Normal;
        }
        self.previous_trend = trend;
        self.adapt(modified, now_us);
    }

    fn adapt(&mut self, modified: f64, now_us: i64) {
        let last = *self.last_threshold_at.get_or_insert(now_us);
        self.last_threshold_at = Some(now_us);
        if modified.abs() > self.threshold + IGNORED_SPIKE_MS {
            return;
        }
        let rate = match modified.abs() < self.threshold {
            true => FALL,
            false => RISE,
        };
        let step = ((now_us - last) as f64 / 1000.0).min(LONGEST_STEP_MS);
        self.threshold += rate * (modified.abs() - self.threshold) * step;
        self.threshold = self
            .threshold
            .clamp(LOWEST_THRESHOLD_MS, HIGHEST_THRESHOLD_MS);
    }
}

fn slope(points: &VecDeque<(f64, f64)>) -> Option<f64> {
    let count = points.len() as f64;
    let mean_x = points.iter().map(|(x, _)| x).sum::<f64>() / count;
    let mean_y = points.iter().map(|(_, y)| y).sum::<f64>() / count;
    let (numerator, denominator) = points.iter().fold((0.0, 0.0), |(top, bottom), (x, y)| {
        (
            top + (x - mean_x) * (y - mean_y),
            bottom + (x - mean_x) * (x - mean_x),
        )
    });
    (denominator != 0.0).then(|| numerator / denominator)
}
