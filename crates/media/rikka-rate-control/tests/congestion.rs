use std::collections::VecDeque;
use std::time::{Duration, Instant};

use rikka_rate_control::{Acknowledgement, Arrival, Congestion, Limits, Sent};

const PACKET: usize = 1200;
const FEEDBACK_EVERY: Duration = Duration::from_millis(25);
const PROPAGATION: Duration = Duration::from_millis(15);
const STEP: Duration = Duration::from_micros(250);

struct Receiver {
    epoch: Instant,
    next_base: u32,
    highest: Option<u32>,
    arrivals: Vec<Arrival>,
}

impl Receiver {
    fn received(&mut self, seq: u32, at: Instant) {
        let at_us = at.duration_since(self.epoch).as_micros() as u32;
        self.arrivals.push(Arrival { seq, at_us });
        self.highest = Some(self.highest.map_or(seq, |highest| highest.max(seq)));
    }

    fn acknowledge(&mut self) -> Option<Acknowledgement<Vec<Arrival>>> {
        let highest = self.highest.filter(|highest| *highest >= self.next_base)?;
        let acknowledgement = Acknowledgement {
            base_seq: self.next_base,
            count: (highest - self.next_base + 1) as u16,
            arrivals: std::mem::take(&mut self.arrivals),
        };
        self.next_base = highest + 1;
        Some(acknowledgement)
    }
}

struct Path {
    capacity_bps: f64,
    busy_until: Instant,
    loss_every: Option<u32>,
    sent: u32,
}

impl Path {
    fn carry(&mut self, at: Instant) -> Option<Instant> {
        self.sent += 1;
        if self
            .loss_every
            .is_some_and(|every| self.sent.is_multiple_of(every))
        {
            return None;
        }
        let start = self.busy_until.max(at);
        self.busy_until = start + Duration::from_secs_f64(PACKET as f64 * 8.0 / self.capacity_bps);
        Some(self.busy_until + PROPAGATION)
    }
}

struct Run {
    congestion: Congestion,
    receiver: Receiver,
    path: Path,
    now: Instant,
    seq: u32,
    next_send: Instant,
    next_feedback: Instant,
    arrivals: VecDeque<(Instant, u32)>,
    probe: Option<(u32, usize, Duration)>,
}

impl Run {
    fn new(capacity_bps: f64, loss_every: Option<u32>) -> Run {
        let now = Instant::now();
        Run {
            congestion: Congestion::new(Limits {
                lowest_bps: 500_000,
                start_bps: 4_000_000,
                highest_bps: 200_000_000,
            }),
            receiver: Receiver {
                epoch: now,
                next_base: 0,
                highest: None,
                arrivals: Vec::new(),
            },
            path: Path {
                capacity_bps,
                busy_until: now,
                loss_every,
                sent: 0,
            },
            now,
            seq: 0,
            next_send: now,
            next_feedback: now + FEEDBACK_EVERY,
            arrivals: VecDeque::new(),
            probe: None,
        }
    }

    fn send(&mut self, probe: Option<u32>) {
        let seq = self.seq;
        self.seq += 1;
        self.congestion.sent(Sent {
            seq,
            at: self.now,
            bytes: PACKET,
            probe,
        });
        if let Some(arrival) = self.path.carry(self.now) {
            self.arrivals.push_back((arrival, seq));
        }
    }

    fn run_for(&mut self, duration: Duration) {
        let end = self.now + duration;
        while self.now < end {
            if self.probe.is_none()
                && let Some(cluster) = self.congestion.probe(self.now, PACKET)
            {
                let gap = Duration::from_secs_f64(PACKET as f64 * 8.0 / cluster.rate_bps as f64);
                self.probe = Some((cluster.id, cluster.packets, gap));
            }
            if let Some((id, left, gap)) = self.probe {
                self.send(Some(id));
                self.probe = (left > 1).then_some((id, left - 1, gap));
                self.now += gap;
                continue;
            }
            while self.now >= self.next_send {
                self.send(None);
                let target = self.congestion.target_bps() as f64;
                self.next_send += Duration::from_secs_f64(PACKET as f64 * 8.0 / target);
            }
            while self.arrivals.front().is_some_and(|(at, _)| *at <= self.now) {
                let (at, seq) = self.arrivals.pop_front().unwrap();
                self.receiver.received(seq, at);
            }
            if self.now >= self.next_feedback {
                self.next_feedback += FEEDBACK_EVERY;
                if let Some(acknowledgement) = self.receiver.acknowledge() {
                    self.congestion
                        .feedback(acknowledgement, self.now, PROPAGATION * 2);
                }
            }
            self.now += STEP;
        }
    }

    fn target_mbps(&self) -> f64 {
        self.congestion.target_bps() as f64 / 1e6
    }
}

#[test]
fn the_estimate_climbs_to_a_free_link_and_stays_under_it() {
    let mut run = Run::new(30e6, None);
    run.run_for(Duration::from_secs(15));
    let target = run.target_mbps();
    assert!(
        (18.0..=34.0).contains(&target),
        "settled at {target} Mbit/s on a 30 Mbit/s link"
    );
}

#[test]
fn the_estimate_follows_a_narrowing_link_down() {
    let mut run = Run::new(30e6, None);
    run.run_for(Duration::from_secs(10));
    run.path.capacity_bps = 6e6;
    run.run_for(Duration::from_secs(3));
    let target = run.target_mbps();
    assert!(target <= 6.6, "still {target} Mbit/s on a 6 Mbit/s link");
    assert!(target >= 2.0, "fell to {target} Mbit/s on a 6 Mbit/s link");
}

#[test]
fn light_random_loss_does_not_collapse_the_estimate() {
    let mut run = Run::new(20e6, Some(40));
    run.run_for(Duration::from_secs(12));
    let target = run.target_mbps();
    assert!(
        target >= 10.0,
        "2.5% loss pushed the estimate to {target} Mbit/s"
    );
}

#[test]
fn heavy_loss_lowers_the_estimate() {
    let mut run = Run::new(20e6, None);
    run.run_for(Duration::from_secs(8));
    let before = run.target_mbps();
    run.path.loss_every = Some(4);
    run.run_for(Duration::from_secs(3));
    let after = run.target_mbps();
    assert!(
        after < before * 0.6,
        "25% loss kept {after} of {before} Mbit/s"
    );
    assert!(run.congestion.loss() > 0.15);
}
