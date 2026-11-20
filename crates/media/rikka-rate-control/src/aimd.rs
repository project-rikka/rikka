use std::time::{Duration, Instant};

use crate::growth::{self, LONGEST_STEP};
use crate::trendline::Usage;

const DECREASE: f64 = 0.85;
const RESPONSE_SLACK: Duration = Duration::from_millis(100);
const ASSUMED_FPS: f64 = 30.0;
const PACKET_BITS: f64 = 1200.0 * 8.0;
const LEAST_ADDITIVE_BPS: f64 = 4000.0;
const LINK_SMOOTHING: f64 = 0.05;
const LINK_SPREAD: f64 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Hold,
    Increase,
    Decrease,
}

#[derive(Clone, Copy, Debug)]
struct Link {
    kbps: f64,
    variance: f64,
}

impl Link {
    fn deviation(&self) -> f64 {
        (self.variance * self.kbps).sqrt()
    }

    fn upper(&self) -> f64 {
        (self.kbps + LINK_SPREAD * self.deviation()) * 1000.0
    }

    fn update(link: Option<Link>, sample_bps: f64) -> Link {
        let sample = sample_bps / 1000.0;
        match link {
            None => Link {
                kbps: sample,
                variance: 0.4,
            },
            Some(link) => {
                let kbps = (1.0 - LINK_SMOOTHING) * link.kbps + LINK_SMOOTHING * sample;
                let error = kbps - sample;
                let variance = ((1.0 - LINK_SMOOTHING) * link.variance
                    + LINK_SMOOTHING * error * error / kbps.max(1.0))
                .clamp(0.4, 2.5);
                Link { kbps, variance }
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Aimd {
    rate: f64,
    lowest: f64,
    highest: f64,
    state: State,
    updated: Option<Instant>,
    link: Option<Link>,
}

impl Aimd {
    pub(crate) fn new(start: f64, lowest: f64, highest: f64) -> Aimd {
        Aimd {
            rate: start.clamp(lowest, highest),
            lowest,
            highest,
            state: State::Hold,
            updated: None,
            link: None,
        }
    }

    pub(crate) fn rate(&self) -> f64 {
        self.rate
    }

    pub(crate) fn raise_to(&mut self, rate: f64) {
        if rate > self.rate {
            self.rate = rate.clamp(self.lowest, self.highest);
        }
    }

    pub(crate) fn set_limits(&mut self, lowest: f64, highest: f64) {
        self.lowest = lowest;
        self.highest = highest;
        self.rate = self.rate.clamp(lowest, highest);
    }

    pub(crate) fn update(
        &mut self,
        usage: Usage,
        acked: Option<f64>,
        now: Instant,
        rtt: Duration,
    ) -> f64 {
        let step = self
            .updated
            .map_or(Duration::ZERO, |updated| {
                now.saturating_duration_since(updated)
            })
            .min(LONGEST_STEP)
            .as_secs_f64();
        self.updated = Some(now);
        self.state = match (usage, self.state) {
            (Usage::Over, _) => State::Decrease,
            (Usage::Under, _) => State::Hold,
            (Usage::Normal, State::Hold) => State::Increase,
            (Usage::Normal, state) => state,
        };
        match self.state {
            State::Hold => {}
            State::Increase => {
                if let (Some(link), Some(acked)) = (self.link, acked)
                    && acked > link.upper()
                {
                    self.link = None;
                }
                let increased = match self.link {
                    Some(_) => self.rate + additive(self.rate, rtt) * step,
                    None => growth::multiplied(self.rate, step),
                };
                self.rate = growth::within_acked(self.rate, increased, acked);
            }
            State::Decrease => {
                let base = acked.unwrap_or(self.rate);
                let mut decreased = DECREASE * base;
                if decreased > self.rate
                    && let Some(link) = self.link
                {
                    decreased = DECREASE * link.kbps * 1000.0;
                }
                self.rate = self.rate.min(decreased);
                if let Some(acked) = acked {
                    self.link = Some(Link::update(self.link, acked));
                }
                self.state = State::Hold;
            }
        }
        self.rate = self.rate.clamp(self.lowest, self.highest);
        self.rate
    }
}

fn additive(rate: f64, rtt: Duration) -> f64 {
    let response_ms = (rtt + RESPONSE_SLACK).as_secs_f64() * 1000.0;
    let frame_bits = rate / ASSUMED_FPS;
    let packets = (frame_bits / PACKET_BITS).ceil().max(1.0);
    let packet_bits = frame_bits / packets;
    (packet_bits * 1000.0 / response_ms).max(LEAST_ADDITIVE_BPS)
}
