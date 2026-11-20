use std::collections::HashMap;

use rikka_remote_wire::{FrameFlags, VIDEO_HEADER_BYTES, VideoShard};

use crate::fec::{Cauchy, FecError, MOST_SHARDS};

pub const MOST_DATA_SHARDS: usize = 100;
const MOST_PROTECTION: f64 = 1.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoFrame {
    pub stream: u16,
    pub frame: u32,
    pub reference: u32,
    pub flags: FrameFlags,
    pub capture_us: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct Packetizer {
    coders: HashMap<(usize, usize), Cauchy>,
    parity: Vec<Vec<u8>>,
    padded: Vec<u8>,
}

impl Packetizer {
    #[must_use]
    pub fn new() -> Packetizer {
        Packetizer::default()
    }

    pub fn packetize(
        &mut self,
        frame: &VideoFrame,
        datagram_bytes: usize,
        protection: f64,
        out: &mut Vec<Vec<u8>>,
    ) -> Result<(), FecError> {
        let room = datagram_bytes.saturating_sub(VIDEO_HEADER_BYTES).max(1);
        let length = frame.data.len().max(1);
        let shards = length.div_ceil(room);
        let shard_bytes = length.div_ceil(shards);
        let blocks = shards.div_ceil(MOST_DATA_SHARDS);
        let per_block = shards.div_ceil(blocks);
        let protection = protection.clamp(0.0, MOST_PROTECTION);
        let frame_bytes = frame.data.len() as u32;
        let mut offset = 0;
        for block in 0..blocks {
            let data = per_block.min(shards - block * per_block);
            let parity = parity_for(data, protection);
            let start = offset;
            let end = (start + data * shard_bytes).min(frame.data.len());
            let bytes = &frame.data[start.min(end)..end];
            self.padded.clear();
            self.padded.extend_from_slice(bytes);
            self.padded.resize(data * shard_bytes, 0);
            let pieces: Vec<&[u8]> = self.padded.chunks(shard_bytes).collect();
            if parity > 0 {
                let coder = match self.coders.entry((data, parity)) {
                    std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(Cauchy::new(data, parity)?)
                    }
                };
                self.parity.resize_with(parity, Vec::new);
                coder.encode(&pieces, &mut self.parity[..parity])?;
            }
            let payloads = pieces
                .iter()
                .copied()
                .chain(self.parity[..parity].iter().map(Vec::as_slice));
            for (index, payload) in payloads.enumerate() {
                let mut datagram = Vec::with_capacity(VIDEO_HEADER_BYTES + payload.len());
                VideoShard {
                    seq: 0,
                    stream: frame.stream,
                    frame: frame.frame,
                    reference: frame.reference,
                    flags: frame.flags,
                    capture_us: frame.capture_us,
                    frame_bytes,
                    block_offset: start as u32,
                    block: block as u16,
                    blocks: blocks as u16,
                    data_shards: data as u8,
                    parity_shards: parity as u8,
                    shard: index as u8,
                    payload,
                }
                .encode(&mut datagram);
                out.push(datagram);
            }
            offset = end;
        }
        Ok(())
    }
}

#[must_use]
pub fn parity_for(data: usize, protection: f64) -> usize {
    if protection <= 0.0 {
        return 0;
    }
    let wanted = (data as f64 * protection).ceil() as usize;
    wanted.max(1).min(MOST_SHARDS - data)
}
