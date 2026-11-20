use std::time::{Duration, Instant};

use crate::progress::Progress;

const SAMPLE_INTERVAL: Duration = Duration::from_millis(200);

const SMOOTHING: f64 = 0.35;

#[derive(Debug)]
pub(crate) struct Meter {
    started: Instant,
    resumed: u64,
    transferred: u64,
    total: Option<u64>,
    rate: Option<f64>,
    sampled_at: Instant,
    sampled_bytes: u64,
}

impl Meter {
    pub(crate) fn new(total: Option<u64>, resumed: u64) -> Meter {
        let now = Instant::now();
        Meter {
            started: now,
            resumed,
            transferred: 0,
            total,
            rate: None,
            sampled_at: now,
            sampled_bytes: 0,
        }
    }

    pub(crate) fn record(&mut self, bytes: u64) {
        self.transferred += bytes;

        let now = Instant::now();
        let elapsed = now.duration_since(self.sampled_at);
        if elapsed < SAMPLE_INTERVAL {
            return;
        }

        #[expect(
            clippy::cast_precision_loss,
            reason = "a transfer rate needs no more precision"
        )]
        let sample = (self.transferred - self.sampled_bytes) as f64 / elapsed.as_secs_f64();

        self.rate = Some(match self.rate {
            Some(rate) => rate.mul_add(1.0 - SMOOTHING, sample * SMOOTHING),
            None => sample,
        });
        self.sampled_at = now;
        self.sampled_bytes = self.transferred;
    }

    pub(crate) fn position(&self) -> u64 {
        self.resumed + self.transferred
    }

    pub(crate) fn progress(&self) -> Progress {
        Progress {
            transferred: self.position(),
            total: self.total,
            bytes_per_second: self.rate.unwrap_or_default(),
            elapsed: self.started.elapsed(),
        }
    }

    pub(crate) fn set_total(&mut self, total: Option<u64>) {
        self.total = total;
    }

    pub(crate) fn resumed_from(&mut self, position: u64) {
        self.resumed = position;
        self.transferred = 0;
        self.sampled_bytes = 0;
        self.sampled_at = Instant::now();
    }
}
