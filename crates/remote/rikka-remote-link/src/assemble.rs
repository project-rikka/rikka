use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, Instant};

use rikka_remote_wire::{FrameFlags, VideoShard};

use crate::fec::Cauchy;
use crate::packetize::VideoFrame;

pub const REORDER_GRACE: Duration = Duration::from_millis(3);
pub const SHARD_SILENCE: Duration = Duration::from_millis(60);
const LONGEST_GAP: u32 = 4096;
const LARGEST_FRAME: u32 = 64 << 20;

struct Block {
    data: usize,
    parity: usize,
    shard_bytes: usize,
    offset: usize,
    present: Vec<bool>,
    parity_shards: Vec<Option<Vec<u8>>>,
    have: usize,
    done: bool,
}

struct Partial {
    reference: u32,
    flags: FrameFlags,
    capture_us: u32,
    data: Vec<u8>,
    blocks: Vec<Option<Block>>,
    blocks_done: usize,
    last_shard: Instant,
}

#[derive(Default)]
pub struct Assembler {
    stream: Option<u16>,
    partial: BTreeMap<u32, Partial>,
    finished_below: Option<u32>,
    finished: BTreeSet<u32>,
    newest_complete: Option<(u32, Instant)>,
    coders: HashMap<(usize, usize), Cauchy>,
}

impl Assembler {
    #[must_use]
    pub fn new() -> Assembler {
        Assembler::default()
    }

    #[must_use]
    pub fn stream(&self) -> Option<u16> {
        self.stream
    }

    pub fn push(&mut self, shard: &VideoShard<'_>, now: Instant) -> Option<VideoFrame> {
        if !self.follows(shard.stream) || self.is_finished(shard.frame) {
            return None;
        }
        if shard.frame_bytes > LARGEST_FRAME {
            return None;
        }
        let blocks = usize::from(shard.blocks);
        let partial = self.partial.entry(shard.frame).or_insert_with(|| Partial {
            reference: shard.reference,
            flags: shard.flags,
            capture_us: shard.capture_us,
            data: vec![0; shard.frame_bytes as usize],
            blocks: (0..blocks).map(|_| None).collect(),
            blocks_done: 0,
            last_shard: now,
        });
        if partial.blocks.len() != blocks || partial.data.len() != shard.frame_bytes as usize {
            return None;
        }
        partial.last_shard = now;
        let slot = &mut partial.blocks[usize::from(shard.block)];
        let block = slot.get_or_insert_with(|| {
            let data = usize::from(shard.data_shards);
            let parity = usize::from(shard.parity_shards);
            Block {
                data,
                parity,
                shard_bytes: shard.payload.len(),
                offset: shard.block_offset as usize,
                present: vec![false; data + parity],
                parity_shards: vec![None; parity],
                have: 0,
                done: false,
            }
        });
        let index = usize::from(shard.shard);
        if block.done
            || block.shard_bytes != shard.payload.len()
            || block.data != usize::from(shard.data_shards)
            || block.parity != usize::from(shard.parity_shards)
            || block.present.get(index) != Some(&false)
        {
            return None;
        }
        block.present[index] = true;
        match index.checked_sub(block.data) {
            None => write_data(block, index, shard.payload, &mut partial.data),
            Some(parity) => block.parity_shards[parity] = Some(shard.payload.to_vec()),
        }
        block.have += 1;
        if block.have < block.data {
            return None;
        }
        if !restore(block, &mut partial.data, &mut self.coders) {
            return None;
        }
        partial.blocks_done += 1;
        if partial.blocks_done < partial.blocks.len() {
            return None;
        }
        let frame = shard.frame;
        let done = self.partial.remove(&frame)?;
        self.finish(frame);
        if self
            .newest_complete
            .is_none_or(|(newest, _)| frame > newest)
        {
            self.newest_complete = Some((frame, now));
        }
        Some(VideoFrame {
            stream: shard.stream,
            frame,
            reference: done.reference,
            flags: done.flags,
            capture_us: done.capture_us,
            data: done.data,
        })
    }

    pub fn expire(&mut self, now: Instant) -> Vec<u32> {
        let mut lost: Vec<u32> = self
            .partial
            .iter()
            .filter(|(_, partial)| {
                now.saturating_duration_since(partial.last_shard) >= SHARD_SILENCE
            })
            .map(|(frame, _)| *frame)
            .collect();
        if let Some((newest, at)) = self.newest_complete
            && now.saturating_duration_since(at) >= REORDER_GRACE
            && let Some(below) = self.finished_below
        {
            let from = below.max(newest.saturating_sub(LONGEST_GAP));
            lost.extend((from..newest).filter(|frame| !self.finished.contains(frame)));
            if from > below {
                self.finished_below = Some(from);
            }
        }
        lost.sort_unstable();
        lost.dedup();
        for frame in &lost {
            self.partial.remove(frame);
            self.finish(*frame);
        }
        lost
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        let silence = self
            .partial
            .values()
            .map(|partial| partial.last_shard + SHARD_SILENCE)
            .min();
        let gap = self.newest_complete.and_then(|(newest, at)| {
            let below = self.finished_below?;
            (below < newest).then_some(at + REORDER_GRACE)
        });
        [silence, gap].into_iter().flatten().min()
    }

    fn follows(&mut self, stream: u16) -> bool {
        match self.stream {
            Some(current) if current == stream => true,
            Some(current) if (stream.wrapping_sub(current) as i16) < 0 => false,
            _ => {
                *self = Assembler {
                    stream: Some(stream),
                    coders: std::mem::take(&mut self.coders),
                    ..Assembler::default()
                };
                true
            }
        }
    }

    fn is_finished(&mut self, frame: u32) -> bool {
        let below = *self.finished_below.get_or_insert(frame);
        frame < below || self.finished.contains(&frame)
    }

    fn finish(&mut self, frame: u32) {
        self.finished.insert(frame);
        let Some(mut below) = self.finished_below else {
            return;
        };
        while self.finished.remove(&below) {
            below += 1;
        }
        self.finished_below = Some(below);
    }
}

impl std::fmt::Debug for Assembler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Assembler")
            .field("stream", &self.stream)
            .field("partial", &self.partial.len())
            .field("finished_below", &self.finished_below)
            .finish_non_exhaustive()
    }
}

fn restore(
    block: &mut Block,
    data: &mut [u8],
    coders: &mut HashMap<(usize, usize), Cauchy>,
) -> bool {
    let complete = block.present[..block.data].iter().all(|present| *present);
    if !complete {
        let coder = match coders.entry((block.data, block.parity)) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                match Cauchy::new(block.data, block.parity) {
                    Ok(coder) => entry.insert(coder),
                    Err(_) => return false,
                }
            }
        };
        let parity = std::mem::take(&mut block.parity_shards);
        let mut shards: Vec<Option<Vec<u8>>> = (0..block.data)
            .map(|index| block.present[index].then(|| read_data(block, index, data)))
            .chain(parity)
            .collect();
        if coder.reconstruct(&mut shards).is_err() {
            return false;
        }
        for (index, shard) in shards[..block.data].iter().enumerate() {
            if let (false, Some(shard)) = (block.present[index], shard) {
                write_data(block, index, shard, data);
            }
        }
    }
    block.parity_shards = Vec::new();
    block.done = true;
    true
}

fn span(block: &Block, index: usize, length: usize) -> Option<std::ops::Range<usize>> {
    let start = block.offset + index * block.shard_bytes;
    (start < length).then(|| start..(start + block.shard_bytes).min(length))
}

fn write_data(block: &Block, index: usize, payload: &[u8], data: &mut [u8]) {
    if let Some(range) = span(block, index, data.len()) {
        let length = range.len();
        data[range].copy_from_slice(&payload[..length]);
    }
}

fn read_data(block: &Block, index: usize, data: &[u8]) -> Vec<u8> {
    let mut shard = vec![0; block.shard_bytes];
    if let Some(range) = span(block, index, data.len()) {
        shard[..range.len()].copy_from_slice(&data[range]);
    }
    shard
}
