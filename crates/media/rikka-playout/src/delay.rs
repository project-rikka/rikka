use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::samples::frames_ms;
use crate::{FRAME, RATE};

const WINDOW: Duration = Duration::from_secs(2);
const BUCKET_MS: f64 = 10.0;
const BUCKETS: usize = 60;
const FORGET: f64 = 0.983;
const START_FORGET: f64 = 2.0;
const QUANTILE: f64 = 0.95;
const MARGIN_MS: f64 = 10.0;
const LOWEST_MS: f64 = 30.0;
const HIGHEST_MS: f64 = 500.0;

#[derive(Debug)]
pub(crate) struct DelayManager {
    transits: VecDeque<(Instant, f64)>,
    histogram: [f64; BUCKETS],
    observed: u32,
    packet_ms: f64,
}

impl DelayManager {
    pub(crate) fn new() -> DelayManager {
        let mut histogram = [0.0; BUCKETS];
        histogram[0] = 1.0;
        DelayManager {
            transits: VecDeque::new(),
            histogram,
            observed: 0,
            packet_ms: frames_ms(FRAME),
        }
    }

    pub(crate) fn observe(
        &mut self,
        arrived: Instant,
        origin: Instant,
        timestamp: u64,
        frames: usize,
    ) {
        self.packet_ms = frames_ms(frames).max(frames_ms(FRAME / 2));
        let sent_ms = timestamp as f64 * 1000.0 / f64::from(RATE);
        let transit = arrived.saturating_duration_since(origin).as_secs_f64() * 1000.0 - sent_ms;
        while self
            .transits
            .front()
            .is_some_and(|(at, _)| arrived.saturating_duration_since(*at) > WINDOW)
        {
            self.transits.pop_front();
        }
        self.transits.push_back((arrived, transit));
        let fastest = self
            .transits
            .iter()
            .map(|(_, transit)| *transit)
            .fold(f64::INFINITY, f64::min);
        let relative = (transit - fastest).max(0.0);
        let bucket = ((relative / BUCKET_MS) as usize).min(BUCKETS - 1);
        let forget = (1.0 - START_FORGET / (f64::from(self.observed) + 1.0)).clamp(0.0, FORGET);
        self.observed = self.observed.saturating_add(1);
        for weight in &mut self.histogram {
            *weight *= forget;
        }
        self.histogram[bucket] += 1.0 - forget;
    }

    pub(crate) fn target(&self) -> Duration {
        let total: f64 = self.histogram.iter().sum();
        let mut cumulative = 0.0;
        let mut quantile = (BUCKETS - 1) as f64 * BUCKET_MS;
        for (index, weight) in self.histogram.iter().enumerate() {
            cumulative += weight;
            if cumulative >= QUANTILE * total {
                quantile = index as f64 * BUCKET_MS;
                break;
            }
        }
        let target_ms = (quantile + self.packet_ms + MARGIN_MS).clamp(LOWEST_MS, HIGHEST_MS);
        Duration::from_secs_f64(target_ms / 1000.0)
    }

    pub(crate) fn talkspurt(&mut self) {
        self.transits.clear();
    }
}
