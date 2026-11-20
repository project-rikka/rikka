use std::collections::HashMap;
use std::time::{Duration, Instant};

use discord_core::VideoCodec;

use crate::codec::Payload;
use crate::rtcp;
use crate::rtp::Header;
use crate::video::assembler::{Assembled, Assembler};

#[derive(Debug, Default)]
pub struct VideoReceiver {
    sources: HashMap<u32, Assembler>,
    repairs: HashMap<u32, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrived {
    pub source: u32,
    pub codec: VideoCodec,
    pub frame: Assembled,
}

impl VideoReceiver {
    pub fn announce(&mut self, primary: u32, repair: Option<u32>) {
        self.repairs.retain(|_, known| *known != primary);
        self.repairs
            .insert(repair.unwrap_or(primary.wrapping_add(1)), primary);
    }

    pub fn forget(&mut self, primary: u32) {
        self.sources.remove(&primary);
        self.repairs.retain(|_, known| *known != primary);
    }

    pub fn packet(
        &mut self,
        header: Header,
        kind: Payload,
        payload: Vec<u8>,
        now: Instant,
    ) -> Vec<Arrived> {
        let (source, codec, sequence, payload) = match kind {
            Payload::Opus => return Vec::new(),
            Payload::Video(codec) => (header.ssrc, codec, header.sequence, payload),
            Payload::Repair(codec) => {
                let Some(original) = payload.get(..2) else {
                    return Vec::new();
                };
                let sequence = u16::from_be_bytes([original[0], original[1]]);
                let source = self
                    .repairs
                    .get(&header.ssrc)
                    .copied()
                    .unwrap_or(header.ssrc.wrapping_sub(1));
                (source, codec, sequence, payload[2..].to_vec())
            }
        };
        let assembler = self
            .sources
            .entry(source)
            .and_modify(|assembler| {
                if assembler.codec() != codec {
                    *assembler = Assembler::new(codec, now);
                }
            })
            .or_insert_with(|| Assembler::new(codec, now));
        if payload.is_empty() {
            assembler.pad(sequence, now);
            return Vec::new();
        }
        assembler
            .insert(sequence, header.timestamp, header.marker, payload, now)
            .into_iter()
            .map(|frame| Arrived {
                source,
                codec,
                frame,
            })
            .collect()
    }

    pub fn broke(&mut self, source: u32) {
        if let Some(assembler) = self.sources.get_mut(&source) {
            assembler.broke();
        }
    }

    pub fn sender_reported(&mut self, source: u32, ntp: u64, now: Instant) {
        if let Some(assembler) = self.sources.get_mut(&source) {
            assembler.sender_reported(ntp, now);
        }
    }

    pub fn feedback(&mut self, me: u32, now: Instant, round_trip: Duration) -> Vec<Vec<u8>> {
        let mut packets = Vec::new();
        for (&source, assembler) in &mut self.sources {
            if let Some(nack) = rtcp::nack(me, source, &assembler.nacks(now, round_trip)) {
                packets.push(nack);
            }
            if assembler.wants_picture(now) {
                packets.push(rtcp::picture_loss(me, source));
            }
        }
        packets
    }

    pub fn report(&mut self, me: u32, now: Instant) -> Option<Vec<u8>> {
        let blocks: Vec<rtcp::ReportBlock> = self
            .sources
            .iter_mut()
            .filter_map(|(&source, assembler)| assembler.report(source, now))
            .collect();
        (!blocks.is_empty()).then(|| rtcp::receiver_report(me, &blocks))
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}
