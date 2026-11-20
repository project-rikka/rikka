use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::probe::ProbeCluster;

const BURST: Duration = Duration::from_millis(5);
const QUANTUM: Duration = Duration::from_millis(1);
const LEAST_RATE_BPS: f64 = 100_000.0;

pub trait Paced {
    fn byte_len(&self) -> usize;
}

impl Paced for Vec<u8> {
    fn byte_len(&self) -> usize {
        self.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outgoing<T> {
    pub packet: T,
    pub probe: Option<u32>,
}

#[derive(Debug)]
struct Probing<T> {
    id: u32,
    rate_bps: f64,
    left: usize,
    padding: T,
    next_at: Instant,
}

#[derive(Debug)]
pub struct Pacer<T> {
    rate_bps: f64,
    budget: f64,
    updated: Option<Instant>,
    urgent: VecDeque<T>,
    video: VecDeque<(u32, T)>,
    video_bytes: usize,
    queued_per_frame: HashMap<u32, usize>,
    probing: Option<Probing<T>>,
}

impl<T: Paced + Clone> Pacer<T> {
    #[must_use]
    pub fn new(rate_bps: u64) -> Pacer<T> {
        Pacer {
            rate_bps: (rate_bps as f64).max(LEAST_RATE_BPS),
            budget: 0.0,
            updated: None,
            urgent: VecDeque::new(),
            video: VecDeque::new(),
            video_bytes: 0,
            queued_per_frame: HashMap::new(),
            probing: None,
        }
    }

    pub fn set_rate(&mut self, rate_bps: u64) {
        self.rate_bps = (rate_bps as f64).max(LEAST_RATE_BPS);
    }

    pub fn push_urgent(&mut self, packet: T) {
        self.urgent.push_back(packet);
    }

    pub fn push_video(&mut self, frame: u32, packet: T) {
        self.video_bytes += packet.byte_len();
        *self.queued_per_frame.entry(frame).or_default() += 1;
        self.video.push_back((frame, packet));
    }

    #[must_use]
    pub fn holds_frame(&self, frame: u32) -> bool {
        self.queued_per_frame.contains_key(&frame)
    }

    pub fn withdraw(&mut self, frame: u32) -> usize {
        let Some(queued) = self.queued_per_frame.remove(&frame) else {
            return 0;
        };
        let mut withdrawn_bytes = 0;
        self.video.retain(|(owner, packet)| {
            let kept = *owner != frame;
            if !kept {
                withdrawn_bytes += packet.byte_len();
            }
            kept
        });
        self.video_bytes -= withdrawn_bytes;
        queued
    }

    pub fn clear(&mut self) {
        self.urgent.clear();
        self.video.clear();
        self.video_bytes = 0;
        self.queued_per_frame.clear();
        self.probing = None;
    }

    #[must_use]
    pub fn queued_video(&self) -> Duration {
        Duration::from_secs_f64(self.video_bytes as f64 * 8.0 / self.rate_bps)
    }

    #[must_use]
    pub fn has_video(&self) -> bool {
        !self.video.is_empty()
    }

    #[must_use]
    pub fn is_probing(&self) -> bool {
        self.probing.is_some()
    }

    pub fn probe(&mut self, cluster: ProbeCluster, padding: T, now: Instant) {
        self.probing = Some(Probing {
            id: cluster.id,
            rate_bps: (cluster.rate_bps as f64).max(LEAST_RATE_BPS),
            left: cluster.packets,
            padding,
            next_at: now,
        });
    }

    pub fn pop(&mut self, now: Instant) -> Option<Outgoing<T>> {
        self.refill(now);
        if let Some(packet) = self.urgent.pop_front() {
            self.budget -= packet.byte_len() as f64;
            return Some(Outgoing {
                packet,
                probe: None,
            });
        }
        if let Some(probing) = &mut self.probing
            && now >= probing.next_at
        {
            let packet = probing.padding.clone();
            let id = probing.id;
            probing.left -= 1;
            probing.next_at +=
                Duration::from_secs_f64(packet.byte_len() as f64 * 8.0 / probing.rate_bps);
            if probing.left == 0 {
                self.probing = None;
            }
            return Some(Outgoing {
                packet,
                probe: Some(id),
            });
        }
        if self.budget <= 0.0 {
            return None;
        }
        let (frame, packet) = self.video.pop_front()?;
        self.video_bytes -= packet.byte_len();
        self.budget -= packet.byte_len() as f64;
        if let Some(queued) = self.queued_per_frame.get_mut(&frame) {
            *queued -= 1;
            if *queued == 0 {
                self.queued_per_frame.remove(&frame);
            }
        }
        Some(Outgoing {
            packet,
            probe: None,
        })
    }

    #[must_use]
    pub fn wake_at(&self, now: Instant) -> Option<Instant> {
        if !self.urgent.is_empty() {
            return Some(now);
        }
        let probe = self.probing.as_ref().map(|probing| probing.next_at);
        let video = (!self.video.is_empty()).then(|| {
            let budget = self.refilled(now);
            match budget > 0.0 {
                true => now,
                false => now + Duration::from_secs_f64(-budget * 8.0 / self.rate_bps).max(QUANTUM),
            }
        });
        [probe, video].into_iter().flatten().min()
    }

    fn refill(&mut self, now: Instant) {
        self.budget = self.refilled(now);
        self.updated = Some(now);
    }

    fn refilled(&self, now: Instant) -> f64 {
        let elapsed = self
            .updated
            .map_or(BURST, |updated| now.saturating_duration_since(updated));
        let rate = self.rate_bps / 8.0;
        (self.budget + rate * elapsed.as_secs_f64()).min(rate * BURST.as_secs_f64())
    }
}
