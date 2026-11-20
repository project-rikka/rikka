use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::buffer::{Buffered, PacketBuffer};
use crate::delay::DelayManager;
use crate::samples::{energy, frames_duration, frames_ms};
use crate::stretch::{Stretch, stretch};
use crate::{CHANNELS, CHUNK, FRAME, RATE};

const RESYNC_FRAMES: u64 = RATE as u64;
const LONGEST_EXPANSION: usize = 6 * FRAME;
const SILENT_ENERGY: f32 = 1.0e-7;
const LEVEL_SMOOTHING: f64 = 1.0 / 16.0;
const HIGH_HEADROOM_MS: f64 = 30.0;
const LOW_HEADROOM_MS: f64 = 25.0;
const LENGTHEN_SPACING: usize = RATE as usize / 5;

pub trait Decode {
    fn frames(&self, packet: &[u8]) -> Option<usize>;

    fn decode(&mut self, packet: &[u8], out: &mut Vec<f32>) -> bool;

    fn recover(&mut self, next: &[u8], frames: usize, out: &mut Vec<f32>) -> bool;

    fn conceal(&mut self, frames: usize, out: &mut Vec<f32>);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub timestamp: u32,
    pub arrived: Instant,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpeechType {
    Silence,
    Concealed,
    Speech,
}

#[derive(Debug)]
enum State {
    Idle,
    Waiting { since: Instant },
    Playing { next: u64 },
}

#[derive(Debug)]
pub struct Playout<D> {
    decoder: D,
    packets: PacketBuffer,
    delay: DelayManager,
    origin: Instant,
    state: State,
    ready: VecDeque<f32>,
    kinds: VecDeque<(usize, SpeechType)>,
    block: Vec<f32>,
    level_ms: f64,
    expanded: usize,
    since_lengthen: usize,
    loud: bool,
}

impl<D: Decode> Playout<D> {
    pub fn new(decoder: D, origin: Instant) -> Playout<D> {
        Playout {
            decoder,
            packets: PacketBuffer::default(),
            delay: DelayManager::new(),
            origin,
            state: State::Idle,
            ready: VecDeque::with_capacity(8 * FRAME * CHANNELS),
            kinds: VecDeque::new(),
            block: Vec::with_capacity(6 * FRAME * CHANNELS),
            level_ms: 0.0,
            expanded: 0,
            since_lengthen: LENGTHEN_SPACING,
            loud: false,
        }
    }

    #[must_use]
    pub fn target(&self) -> Duration {
        self.delay.target()
    }

    #[must_use]
    pub fn buffered(&self) -> Duration {
        frames_duration(self.ready.len() / CHANNELS + self.packets.frames())
    }

    pub fn push(&mut self, packet: Packet) {
        let Some(frames) = self
            .decoder
            .frames(&packet.payload)
            .filter(|frames| *frames > 0)
        else {
            return;
        };
        let timestamp = self.packets.unwrap(packet.timestamp);
        if matches!(self.state, State::Idle) {
            self.delay.talkspurt();
            self.state = State::Waiting {
                since: packet.arrived,
            };
        }
        self.delay
            .observe(packet.arrived, self.origin, timestamp, frames);
        if let State::Playing { next } = self.state
            && timestamp < next
        {
            return;
        }
        self.packets.insert(
            timestamp,
            Buffered {
                payload: packet.payload,
                frames,
            },
        );
    }

    pub fn pull(&mut self, now: Instant, out: &mut [f32]) -> SpeechType {
        while self.ready.len() < out.len() && self.step(now) {}
        let mut speech = SpeechType::Silence;
        let available = self.ready.len().min(out.len());
        for (slot, sample) in out.iter_mut().zip(self.ready.drain(..available)) {
            *slot = sample;
        }
        out[available..].fill(0.0);
        let mut remaining = available / CHANNELS;
        while remaining > 0 {
            let Some((frames, kind)) = self.kinds.front_mut() else {
                break;
            };
            speech = speech.max(*kind);
            let count = (*frames).min(remaining);
            *frames -= count;
            remaining -= count;
            if *frames == 0 {
                self.kinds.pop_front();
            }
        }
        self.since_lengthen = self.since_lengthen.saturating_add(out.len() / CHANNELS);
        let level = self.buffered().as_secs_f64() * 1000.0;
        self.level_ms += (level - self.level_ms) * LEVEL_SMOOTHING;
        speech
    }

    fn step(&mut self, now: Instant) -> bool {
        match self.state {
            State::Idle => false,
            State::Waiting { since } => self.start(now, since),
            State::Playing { next } => self.play(next),
        }
    }

    fn start(&mut self, now: Instant, since: Instant) -> bool {
        let Some((first, _)) = self.packets.first() else {
            self.state = State::Idle;
            return false;
        };
        let target = self.delay.target();
        let deep = frames_duration(self.packets.frames()) >= target;
        if !deep && now.saturating_duration_since(since) < target {
            return false;
        }
        self.state = State::Playing { next: first };
        self.expanded = 0;
        self.level_ms = target.as_secs_f64() * 1000.0;
        true
    }

    fn play(&mut self, next: u64) -> bool {
        self.packets.drop_before(next);
        self.block.clear();
        if let Some(packet) = self.packets.take(next) {
            if !self.decoder.decode(&packet.payload, &mut self.block) {
                self.decoder.conceal(packet.frames, &mut self.block);
            }
            let decoded = (self.block.len() / CHANNELS).max(1);
            self.state = State::Playing {
                next: next + decoded as u64,
            };
            self.expanded = 0;
            self.loud = energy(&self.block) > SILENT_ENERGY;
            self.adjust();
            self.queue(SpeechType::Speech);
            return true;
        }
        if let Some((first, following)) = self.packets.first() {
            let gap = first - next;
            if gap > RESYNC_FRAMES {
                self.state = State::Playing { next: first };
                return true;
            }
            let lost = (gap as usize).min(FRAME);
            let recovered = first == next + lost as u64
                && self
                    .decoder
                    .recover(&following.payload, lost, &mut self.block);
            if !recovered {
                self.block.clear();
                self.decoder.conceal(lost, &mut self.block);
            }
            self.state = State::Playing {
                next: next + lost as u64,
            };
            self.queue(SpeechType::Concealed);
            return true;
        }
        if !self.loud || self.expanded >= LONGEST_EXPANSION {
            self.state = State::Idle;
            self.loud = false;
            return false;
        }
        self.decoder.conceal(CHUNK, &mut self.block);
        self.expanded += CHUNK;
        self.queue(SpeechType::Concealed);
        true
    }

    fn adjust(&mut self) {
        let target = self.delay.target().as_secs_f64() * 1000.0;
        let high = (target + HIGH_HEADROOM_MS).max(target * 1.5);
        let low = target - LOW_HEADROOM_MS;
        let how = if self.level_ms > high {
            Stretch::Shorten
        } else if self.level_ms < low
            && !self.packets.is_empty()
            && self.since_lengthen >= LENGTHEN_SPACING
        {
            Stretch::Lengthen
        } else {
            return;
        };
        let before = self.block.len() / CHANNELS;
        if stretch(&mut self.block, how) {
            let after = self.block.len() / CHANNELS;
            self.level_ms += frames_ms(after) - frames_ms(before);
            if how == Stretch::Lengthen {
                self.since_lengthen = 0;
            }
        }
    }

    fn queue(&mut self, kind: SpeechType) {
        let frames = self.block.len() / CHANNELS;
        if frames == 0 {
            return;
        }
        self.ready.extend(self.block.iter().copied());
        match self.kinds.back_mut() {
            Some((counted, last)) if *last == kind => *counted += frames,
            _ => self.kinds.push_back((frames, kind)),
        }
    }
}
