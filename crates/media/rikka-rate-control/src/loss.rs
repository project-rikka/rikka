use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::growth::{self, LONGEST_STEP};
use crate::limits::Limits;

const LOSS_WINDOW: Duration = Duration::from_secs(1);
const LEAST_COUNTED: usize = 10;
const LOW_LOSS: f64 = 0.02;
const HIGH_LOSS: f64 = 0.10;
const LOSS_BACKOFF: f64 = 0.5;
const DECREASE_PAUSE: Duration = Duration::from_millis(300);

#[derive(Debug)]
pub struct LossBased {
    rate: f64,
    lowest: f64,
    highest: f64,
    receiver_estimate: Option<f64>,
    window: VecDeque<(Instant, bool)>,
    fraction: f64,
    updated: Option<Instant>,
    decreased: Option<Instant>,
}

impl LossBased {
    #[must_use]
    pub fn new(limits: Limits) -> LossBased {
        let rates = limits.rates();
        LossBased {
            rate: rates.start,
            lowest: rates.lowest,
            highest: rates.highest,
            receiver_estimate: None,
            window: VecDeque::new(),
            fraction: 0.0,
            updated: None,
            decreased: None,
        }
    }

    pub fn set_limits(&mut self, limits: Limits) {
        let rates = limits.rates();
        self.lowest = rates.lowest;
        self.highest = rates.highest;
        self.rate = self.rate.clamp(self.lowest, self.ceiling());
    }

    pub fn set_receiver_estimate(&mut self, estimate_bps: u64) {
        self.receiver_estimate = Some(estimate_bps as f64);
        self.rate = self.rate.clamp(self.lowest, self.ceiling());
    }

    #[must_use]
    pub fn rate_bps(&self) -> u64 {
        self.rate as u64
    }

    #[must_use]
    pub fn highest_bps(&self) -> u64 {
        self.ceiling() as u64
    }

    #[must_use]
    pub fn loss(&self) -> f64 {
        self.fraction
    }

    pub fn raise_to(&mut self, rate_bps: u64) {
        self.raise(rate_bps as f64);
    }

    pub fn report(&mut self, fraction_lost: f64, now: Instant, rtt: Duration) -> u64 {
        self.fraction = fraction_lost.clamp(0.0, 1.0);
        self.adjust(None, now, rtt) as u64
    }

    pub(crate) fn rate(&self) -> f64 {
        self.rate
    }

    pub(crate) fn raise(&mut self, rate: f64) {
        if rate > self.rate {
            self.rate = rate.clamp(self.lowest, self.ceiling());
        }
    }

    pub(crate) fn update(
        &mut self,
        outcomes: impl Iterator<Item = bool>,
        acked: Option<f64>,
        now: Instant,
        rtt: Duration,
    ) -> f64 {
        self.window.extend(outcomes.map(|lost| (now, lost)));
        while self
            .window
            .front()
            .is_some_and(|(at, _)| now.saturating_duration_since(*at) > LOSS_WINDOW)
        {
            self.window.pop_front();
        }
        if self.window.len() >= LEAST_COUNTED {
            let lost = self.window.iter().filter(|(_, lost)| *lost).count();
            self.fraction = lost as f64 / self.window.len() as f64;
        }
        self.adjust(acked, now, rtt)
    }

    fn adjust(&mut self, acked: Option<f64>, now: Instant, rtt: Duration) -> f64 {
        let step = self
            .updated
            .map_or(Duration::ZERO, |updated| {
                now.saturating_duration_since(updated)
            })
            .min(LONGEST_STEP)
            .as_secs_f64();
        self.updated = Some(now);
        if self.fraction < LOW_LOSS {
            let increased = growth::multiplied(self.rate, step);
            self.rate = growth::within_acked(self.rate, increased, acked);
        } else if self.fraction > HIGH_LOSS
            && self
                .decreased
                .is_none_or(|at| now.saturating_duration_since(at) >= DECREASE_PAUSE + rtt)
        {
            self.rate *= 1.0 - LOSS_BACKOFF * self.fraction;
            self.decreased = Some(now);
        }
        self.rate = self.rate.clamp(self.lowest, self.ceiling());
        self.rate
    }

    fn ceiling(&self) -> f64 {
        self.receiver_estimate
            .map_or(self.highest, |estimate| estimate.min(self.highest))
            .max(self.lowest)
    }
}
