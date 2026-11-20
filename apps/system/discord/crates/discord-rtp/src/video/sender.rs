use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use discord_core::VIDEO_CLOCK;

use crate::codec::{Payload, SENT_CODEC};
use crate::rtcp::{self, SenderInfo};
use crate::rtp::{Fields, HEADER_LEN, ONE_BYTE_PROFILE};
use crate::video::extensions;

const HISTORY: usize = 2048;
const PREAMBLE: usize = 4;
const CLEAR_LEN: usize = HEADER_LEN + PREAMBLE;
const SEALING: usize = PREAMBLE + 16 + 16 + 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plain {
    pub(crate) fields: Fields,
    pub(crate) repaired: bool,
    pub(crate) payload: Arc<[u8]>,
}

impl rikka_rate_control::Paced for Plain {
    fn byte_len(&self) -> usize {
        HEADER_LEN + SEALING + self.payload.len()
    }
}

impl Plain {
    #[must_use]
    pub fn packet(&self, transport: u16) -> Vec<u8> {
        let elements = extensions::video(transport, self.repaired);
        let words = (elements.len() / 4) as u16;
        let mut packet = Vec::with_capacity(CLEAR_LEN + elements.len() + self.payload.len());
        packet.extend_from_slice(&self.fields.header(true));
        packet.extend_from_slice(&ONE_BYTE_PROFILE);
        packet.extend_from_slice(&words.to_be_bytes());
        packet.extend_from_slice(&elements);
        packet.extend_from_slice(&self.payload);
        packet
    }
}

#[derive(Debug)]
pub struct VideoSender {
    ssrc: u32,
    repair_ssrc: u32,
    sequence: u16,
    repair_sequence: u16,
    base: u32,
    origin: Option<Duration>,
    history: VecDeque<Plain>,
    packets: u32,
    octets: u32,
    last: Option<(u32, u64)>,
}

impl VideoSender {
    #[must_use]
    pub fn new(ssrc: u32, repair_ssrc: u32) -> VideoSender {
        VideoSender {
            ssrc,
            repair_ssrc,
            sequence: rand::random(),
            repair_sequence: rand::random(),
            base: rand::random(),
            origin: None,
            history: VecDeque::with_capacity(HISTORY),
            packets: 0,
            octets: 0,
            last: None,
        }
    }

    #[must_use]
    pub fn ssrc(&self) -> u32 {
        self.ssrc
    }

    #[must_use]
    pub fn repair_ssrc(&self) -> u32 {
        self.repair_ssrc
    }

    pub fn frame(&mut self, payloads: Vec<Vec<u8>>, captured: Duration) -> Vec<Plain> {
        let origin = *self.origin.get_or_insert(captured);
        let elapsed = captured.saturating_sub(origin);
        let ticks = elapsed.as_micros() * u128::from(VIDEO_CLOCK) / 1_000_000;
        let timestamp = self.base.wrapping_add(ticks as u32);
        self.last = Some((timestamp, rtcp::ntp_now()));
        let count = payloads.len();
        let mut plains = Vec::with_capacity(count);
        for (index, payload) in payloads.into_iter().enumerate() {
            let plain = Plain {
                fields: Fields {
                    marker: index + 1 == count,
                    payload_type: Payload::Video(SENT_CODEC).payload_type(),
                    sequence: self.sequence,
                    timestamp,
                    ssrc: self.ssrc,
                },
                repaired: false,
                payload: payload.into(),
            };
            self.sequence = self.sequence.wrapping_add(1);
            self.packets = self.packets.wrapping_add(1);
            self.octets = self.octets.wrapping_add(plain.payload.len() as u32);
            if self.history.len() == HISTORY {
                self.history.pop_front();
            }
            self.history.push_back(plain.clone());
            plains.push(plain);
        }
        plains
    }

    pub fn repair(&mut self, sequence: u16) -> Option<Plain> {
        let newest = self.history.back()?.fields.sequence;
        let back = usize::from(newest.wrapping_sub(sequence));
        let index = self.history.len().checked_sub(back + 1)?;
        let original = self.history.get(index)?;
        if original.fields.sequence != sequence {
            return None;
        }
        let mut payload = Vec::with_capacity(original.payload.len() + 2);
        payload.extend_from_slice(&sequence.to_be_bytes());
        payload.extend_from_slice(&original.payload);
        let repaired = Plain {
            fields: Fields {
                payload_type: Payload::Repair(SENT_CODEC).payload_type(),
                sequence: self.repair_sequence,
                ssrc: self.repair_ssrc,
                ..original.fields
            },
            repaired: true,
            payload: payload.into(),
        };
        self.repair_sequence = self.repair_sequence.wrapping_add(1);
        Some(repaired)
    }

    #[must_use]
    pub fn report(&self) -> Option<SenderInfo> {
        let (timestamp, ntp) = self.last?;
        Some(SenderInfo {
            source: self.ssrc,
            ntp,
            timestamp,
            packets: self.packets,
            octets: self.octets,
        })
    }
}
