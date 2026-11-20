use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use discord_core::VideoCodec;

use crate::rtcp::ReportBlock;
use crate::video::depacketize::{self, Parsed};
use crate::video::reception::Reception;

const ORIGIN: u64 = 1 << 32;
const GIVE_UP: Duration = Duration::from_millis(1000);
const MAX_TRIES: u8 = 10;
const MIN_RETRY: Duration = Duration::from_millis(30);
const PICTURE_EVERY: Duration = Duration::from_millis(500);
const MAX_PACKETS: usize = 4096;
const MAX_GAP: u64 = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assembled {
    pub timestamp: u32,
    pub keyframe: bool,
    pub data: Vec<u8>,
}

#[derive(Debug)]
struct Stored {
    timestamp: u32,
    marker: bool,
    parsed: Parsed,
    payload: Vec<u8>,
}

#[derive(Debug)]
struct Asked {
    since: Instant,
    last: Option<Instant>,
    tries: u8,
}

#[derive(Debug)]
pub(crate) struct Assembler {
    codec: VideoCodec,
    packets: BTreeMap<u64, Stored>,
    highest: Option<u64>,
    next: Option<u64>,
    missing: BTreeMap<u64, Asked>,
    padding: BTreeSet<u64>,
    awaiting_keyframe: bool,
    last_picture_request: Option<Instant>,
    reception: Reception,
}

impl Assembler {
    pub(crate) fn new(codec: VideoCodec, now: Instant) -> Assembler {
        Assembler {
            codec,
            packets: BTreeMap::new(),
            highest: None,
            next: None,
            missing: BTreeMap::new(),
            padding: BTreeSet::new(),
            awaiting_keyframe: true,
            last_picture_request: None,
            reception: Reception::new(now),
        }
    }

    pub(crate) fn codec(&self) -> VideoCodec {
        self.codec
    }

    pub(crate) fn insert(
        &mut self,
        sequence: u16,
        timestamp: u32,
        marker: bool,
        payload: Vec<u8>,
        now: Instant,
    ) -> Vec<Assembled> {
        let extended = self.extend(sequence);
        if self.next.is_some_and(|next| extended < next) || self.packets.contains_key(&extended) {
            return Vec::new();
        }
        let Some(parsed) = depacketize::parse(self.codec, &payload) else {
            return Vec::new();
        };
        self.reception.arrived(extended, timestamp, now);
        if !self.note_arrival(extended, now) {
            return Vec::new();
        }
        if self.next.is_none() {
            self.next = match parsed.starts {
                Some(true) => Some(extended),
                Some(false) => None,
                None => marker.then_some(extended + 1),
            };
        } else if self.awaiting_keyframe && parsed.keyframe && parsed.starts == Some(true) {
            self.skip_to(extended);
        }
        if self.next.is_none_or(|next| extended < next) {
            return Vec::new();
        }
        self.packets.insert(
            extended,
            Stored {
                timestamp,
                marker,
                parsed,
                payload,
            },
        );
        let mut assembled = self.drain();
        if self.stalled(now) {
            self.recover();
            assembled.extend(self.drain());
        }
        assembled
    }

    pub(crate) fn pad(&mut self, sequence: u16, now: Instant) -> Vec<Assembled> {
        let extended = self.extend(sequence);
        let Some(next) = self.next else {
            self.note_arrival(extended, now);
            return Vec::new();
        };
        if extended < next {
            return Vec::new();
        }
        self.note_arrival(extended, now);
        self.padding.insert(extended);
        self.drain()
    }

    pub(crate) fn broke(&mut self) {
        self.awaiting_keyframe = true;
    }

    pub(crate) fn wants_picture(&mut self, now: Instant) -> bool {
        if !self.awaiting_keyframe {
            return false;
        }
        let due = self.last_picture_request.is_none_or(|last_request| {
            now.saturating_duration_since(last_request) >= PICTURE_EVERY
        });
        if due {
            self.last_picture_request = Some(now);
        }
        due
    }

    pub(crate) fn nacks(&mut self, now: Instant, round_trip: Duration) -> Vec<u16> {
        let retry = round_trip.mul_f32(1.5).max(MIN_RETRY);
        let floor = self.next.unwrap_or(0);
        let mut lost = Vec::new();
        for (&extended, request) in self.missing.range_mut(floor..) {
            if request.tries >= MAX_TRIES || now.saturating_duration_since(request.since) > GIVE_UP
            {
                continue;
            }
            let due = request
                .last
                .is_none_or(|last| now.saturating_duration_since(last) >= retry);
            if due {
                request.tries += 1;
                request.last = Some(now);
                lost.push(extended as u16);
            }
        }
        lost
    }

    pub(crate) fn sender_reported(&mut self, ntp: u64, now: Instant) {
        self.reception.sender_reported(ntp, now);
    }

    pub(crate) fn report(&mut self, source: u32, now: Instant) -> Option<ReportBlock> {
        self.reception.block(source, now)
    }

    fn extend(&self, sequence: u16) -> u64 {
        match self.highest {
            None => ORIGIN + u64::from(sequence),
            Some(highest) => {
                let delta = i64::from(sequence.wrapping_sub(highest as u16) as i16);
                highest.saturating_add_signed(delta)
            }
        }
    }

    fn note_arrival(&mut self, extended: u64, now: Instant) -> bool {
        self.missing.remove(&extended);
        let Some(highest) = self.highest else {
            self.highest = Some(extended);
            return true;
        };
        if extended <= highest {
            return true;
        }
        if extended - highest > MAX_GAP {
            self.packets.clear();
            self.missing.clear();
            self.next = None;
            self.awaiting_keyframe = true;
            self.highest = Some(extended);
            return true;
        }
        for gap in highest + 1..extended {
            self.missing.insert(
                gap,
                Asked {
                    since: now,
                    last: None,
                    tries: 0,
                },
            );
        }
        self.highest = Some(extended);
        true
    }

    fn drain(&mut self) -> Vec<Assembled> {
        let mut assembled = Vec::new();
        loop {
            if let Some(next) = self.next
                && self.padding.remove(&next)
            {
                self.next = Some(next + 1);
                continue;
            }
            let Some((start, end)) = self.complete() else {
                break;
            };
            let frame = self.take(start, end);
            self.next = Some(end + 1);
            self.missing = self.missing.split_off(&(end + 1));
            self.padding = self.padding.split_off(&(end + 1));
            if let Some(frame) = frame {
                assembled.push(frame);
            }
        }
        assembled
    }

    fn complete(&self) -> Option<(u64, u64)> {
        let start = self.next?;
        let first = self.packets.get(&start)?;
        if first.parsed.starts == Some(false) {
            return None;
        }
        let mut end = start;
        loop {
            let packet = self.packets.get(&end)?;
            if packet.timestamp != first.timestamp {
                return Some((start, end - 1));
            }
            if packet.marker {
                return Some((start, end));
            }
            if let Some(following) = self.packets.get(&(end + 1))
                && following.parsed.starts == Some(true)
                && following.timestamp != first.timestamp
            {
                return Some((start, end));
            }
            end += 1;
        }
    }

    fn take(&mut self, start: u64, end: u64) -> Option<Assembled> {
        let mut rest = self.packets.split_off(&(end + 1));
        std::mem::swap(&mut rest, &mut self.packets);
        let frame: Vec<Stored> = rest.into_values().collect();
        let first = frame.first()?;
        let timestamp = first.timestamp;
        let keyframe = match first.parsed.starts {
            Some(_) => first.parsed.keyframe,
            None => frame.iter().any(|packet| packet.parsed.keyframe),
        };
        debug_assert_eq!(frame.len() as u64, end - start + 1);
        if self.awaiting_keyframe && !keyframe {
            return None;
        }
        let payloads: Vec<&[u8]> = frame
            .iter()
            .map(|packet| packet.payload.as_slice())
            .collect();
        let Some(data) = depacketize::assemble(self.codec, &payloads) else {
            self.awaiting_keyframe = true;
            return None;
        };
        if keyframe {
            self.awaiting_keyframe = false;
        }
        Some(Assembled {
            timestamp,
            keyframe,
            data,
        })
    }

    fn stalled(&self, now: Instant) -> bool {
        if self.packets.len() > MAX_PACKETS {
            return true;
        }
        let floor = self.next.unwrap_or(0);
        self.missing
            .range(floor..)
            .next()
            .is_some_and(|(_, request)| now.saturating_duration_since(request.since) > GIVE_UP)
    }

    fn recover(&mut self) {
        let floor = self.next.unwrap_or(0);
        let past = self
            .missing
            .range(floor..)
            .next()
            .map(|(&gap, _)| gap)
            .or_else(|| self.packets.keys().next().copied())
            .unwrap_or(floor);
        let resume = self
            .packets
            .range(past + 1..)
            .find_map(|(&extended, packet)| match packet.parsed.starts {
                Some(true) => Some(extended),
                Some(false) => None,
                None => packet.marker.then_some(extended + 1),
            });
        self.awaiting_keyframe = true;
        match resume {
            Some(resume) => self.skip_to(resume),
            None => {
                self.packets.clear();
                self.missing = self.highest.map_or_else(BTreeMap::new, |highest| {
                    self.missing.split_off(&(highest + 1))
                });
                self.next = self.highest.map(|highest| highest + 1);
            }
        }
    }

    fn skip_to(&mut self, resume: u64) {
        self.padding = self.padding.split_off(&resume);
        self.packets = self.packets.split_off(&resume);
        self.missing = self.missing.split_off(&resume);
        self.next = Some(resume);
    }
}
