use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::acked::Acked;
use crate::aimd::Aimd;
use crate::history::{Acknowledgement, Arrival, History, Sent};
use crate::inter_arrival::InterArrival;
use crate::limits::Limits;
use crate::loss::LossBased;
use crate::probe::{LEAST_PROBE_PACKETS, ProbeCluster, ProbeResults};
use crate::trendline::{Trendline, Usage};

const FIRST_PROBE: f64 = 3.0;
const NEXT_PROBE: f64 = 2.0;
const PROBE_SUCCESS: f64 = 0.7;
const PROBE_PATIENCE: Duration = Duration::from_secs(1);
const PROBE_SPAN: Duration = Duration::from_millis(15);
const MOST_PROBE_PACKETS: usize = 100;
const LIMITED_SHARE: f64 = 0.5;
const LIMITED_FOR: Duration = Duration::from_secs(1);
const PROBE_EVERY: Duration = Duration::from_secs(5);
const SENT_WINDOW: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Update {
    pub target_bps: u64,
    pub acked_bps: Option<u64>,
    pub loss: f64,
    pub usage: Usage,
}

#[derive(Clone, Copy, Debug)]
enum Planner {
    Start,
    Waiting { rate: f64, since: Instant },
    Next { rate: f64 },
    Idle { last: Option<Instant> },
}

#[derive(Debug)]
pub struct Congestion {
    limits: Limits,
    history: History,
    arrivals: InterArrival,
    trendline: Trendline,
    aimd: Aimd,
    loss: LossBased,
    acked: Acked,
    probes: ProbeResults,
    planner: Planner,
    next_probe: u32,
    sent: VecDeque<(Instant, usize)>,
    limited_since: Option<Instant>,
    usage: Usage,
    target: f64,
}

impl Congestion {
    #[must_use]
    pub fn new(limits: Limits) -> Congestion {
        let rates = limits.rates();
        Congestion {
            limits,
            history: History::new(),
            arrivals: InterArrival::default(),
            trendline: Trendline::default(),
            aimd: Aimd::new(rates.start, rates.lowest, rates.highest),
            loss: LossBased::new(limits),
            acked: Acked::default(),
            probes: ProbeResults::default(),
            planner: Planner::Start,
            next_probe: 0,
            sent: VecDeque::new(),
            limited_since: None,
            usage: Usage::Normal,
            target: rates.start,
        }
    }

    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
        let rates = limits.rates();
        self.aimd.set_limits(rates.lowest, rates.highest);
        self.loss.set_limits(limits);
        self.target = self.aimd.rate().min(self.loss.rate());
    }

    #[must_use]
    pub fn target_bps(&self) -> u64 {
        self.target as u64
    }

    #[must_use]
    pub fn loss(&self) -> f64 {
        self.loss.loss()
    }

    pub fn sent(&mut self, packet: Sent) {
        self.history.sent(packet);
        self.sent.push_back((packet.at, packet.bytes));
        while self
            .sent
            .front()
            .is_some_and(|(at, _)| packet.at.saturating_duration_since(*at) > SENT_WINDOW)
        {
            self.sent.pop_front();
        }
    }

    pub fn feedback<I>(
        &mut self,
        acknowledgement: Acknowledgement<I>,
        now: Instant,
        rtt: Duration,
    ) -> Update
    where
        I: IntoIterator<Item = Arrival>,
    {
        let results = self.history.acknowledge(acknowledgement);
        for result in &results {
            let Some(arrival) = result.arrival_us else {
                continue;
            };
            self.acked.add(arrival, result.sent.bytes);
            if let Some(delta) = self.arrivals.add(result.sent.at, arrival) {
                self.usage = self.trendline.update(delta);
            }
        }
        let acked = self.acked.rate();
        let delay = self.aimd.update(self.usage, acked, now, rtt);
        let loss = self.loss.update(
            results.iter().map(|result| result.arrival_us.is_none()),
            acked,
            now,
            rtt,
        );
        self.target = delay.min(loss);
        if let Some(probed) = self.probes.add(&results) {
            self.probed(probed, now);
        }
        Update {
            target_bps: self.target as u64,
            acked_bps: acked.map(|rate| rate as u64),
            loss: self.loss.loss(),
            usage: self.usage,
        }
    }

    pub fn probe(&mut self, now: Instant, packet_bytes: usize) -> Option<ProbeCluster> {
        let rates = self.limits.rates();
        let limited = self.sending_rate(now) < LIMITED_SHARE * self.target;
        self.limited_since = match limited {
            true => Some(self.limited_since.unwrap_or(now)),
            false => None,
        };
        let rate = match self.planner {
            Planner::Start => Some(FIRST_PROBE * rates.start),
            Planner::Next { rate } => Some(rate),
            Planner::Waiting { since, .. } => {
                if now.saturating_duration_since(since) > PROBE_PATIENCE {
                    self.planner = Planner::Idle { last: Some(since) };
                }
                None
            }
            Planner::Idle { last } => {
                let due =
                    last.is_none_or(|last| now.saturating_duration_since(last) >= PROBE_EVERY);
                let quiet = self
                    .limited_since
                    .is_some_and(|since| now.saturating_duration_since(since) >= LIMITED_FOR);
                (due && quiet && self.target < rates.highest).then_some(NEXT_PROBE * self.target)
            }
        }?;
        let rate = rate.min(rates.highest);
        if rate <= self.target {
            self.planner = Planner::Idle { last: Some(now) };
            return None;
        }
        self.planner = Planner::Waiting { rate, since: now };
        let bits = (packet_bytes.max(1) * 8) as f64;
        let packets = ((rate * PROBE_SPAN.as_secs_f64() / bits).ceil() as usize)
            .clamp(LEAST_PROBE_PACKETS, MOST_PROBE_PACKETS);
        self.next_probe = self.next_probe.wrapping_add(1);
        let cluster = ProbeCluster {
            id: self.next_probe,
            rate_bps: rate as u64,
            packets,
        };
        self.probes.planned(cluster);
        Some(cluster)
    }

    fn probed(&mut self, estimate: f64, now: Instant) {
        if estimate > self.target {
            self.aimd.raise_to(estimate);
            self.loss.raise(estimate);
            self.target = self.aimd.rate().min(self.loss.rate());
        }
        let highest = self.limits.rates().highest;
        self.planner = match self.planner {
            Planner::Waiting { rate, .. } if estimate >= PROBE_SUCCESS * rate && rate < highest => {
                Planner::Next {
                    rate: NEXT_PROBE * estimate,
                }
            }
            _ => Planner::Idle { last: Some(now) },
        };
    }

    fn sending_rate(&self, now: Instant) -> f64 {
        let Some((first, _)) = self.sent.front() else {
            return 0.0;
        };
        let span = now.saturating_duration_since(*first).max(SENT_WINDOW / 4);
        let bytes: usize = self.sent.iter().map(|(_, bytes)| bytes).sum();
        bytes as f64 * 8.0 / span.as_secs_f64()
    }
}
