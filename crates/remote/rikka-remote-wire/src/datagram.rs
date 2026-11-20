use crate::bytes::{Reader, Writer};
use crate::error::DecodeError;
use crate::input::Position;

pub const VIDEO_HEADER_BYTES: usize = 38;
pub const AUDIO_HEADER_BYTES: usize = 10;
pub const NO_FRAME: u32 = u32::MAX;
pub const DELTA_UNIT_US: u32 = 50;

const VIDEO: u8 = 0x01;
const AUDIO: u8 = 0x02;
const PADDING: u8 = 0x03;
const POINTER: u8 = 0x04;
const MOTION: u8 = 0x41;
const FEEDBACK: u8 = 0x42;

const INTRA: u8 = 0b01;
const REFINEMENT: u8 = 0b10;
const REDUNDANT: u8 = 0b1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FrameFlags {
    pub intra: bool,
    pub refinement: bool,
}

impl FrameFlags {
    fn bits(self) -> u8 {
        (if self.intra { INTRA } else { 0 }) | (if self.refinement { REFINEMENT } else { 0 })
    }

    fn from_bits(bits: u8) -> FrameFlags {
        FrameFlags {
            intra: bits & INTRA != 0,
            refinement: bits & REFINEMENT != 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoShard<'a> {
    pub seq: u32,
    pub stream: u16,
    pub frame: u32,
    pub reference: u32,
    pub flags: FrameFlags,
    pub capture_us: u32,
    pub frame_bytes: u32,
    pub block_offset: u32,
    pub block: u16,
    pub blocks: u16,
    pub data_shards: u8,
    pub parity_shards: u8,
    pub shard: u8,
    pub payload: &'a [u8],
}

impl VideoShard<'_> {
    pub fn encode(&self, out: &mut Vec<u8>) {
        Writer(out)
            .u8(VIDEO)
            .u32(self.seq)
            .u16(self.stream)
            .u32(self.frame)
            .u32(self.reference)
            .u8(self.flags.bits())
            .u32(self.capture_us)
            .u32(self.frame_bytes)
            .u32(self.block_offset)
            .u16(self.block)
            .u16(self.blocks)
            .u8(self.data_shards)
            .u8(self.parity_shards)
            .u8(self.shard)
            .u8(0)
            .u16(self.payload.len() as u16)
            .raw(self.payload);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioPacket<'a> {
    pub seq: u32,
    pub timestamp: u32,
    pub primary: &'a [u8],
    pub previous: Option<&'a [u8]>,
}

impl AudioPacket<'_> {
    pub fn encode(&self, out: &mut Vec<u8>) {
        let mut writer = Writer(out);
        writer.u8(AUDIO).u32(self.seq).u32(self.timestamp);
        match self.previous {
            Some(previous) => {
                writer
                    .u8(REDUNDANT)
                    .u16(self.primary.len() as u16)
                    .raw(self.primary)
                    .raw(previous);
            }
            None => {
                writer.u8(0).raw(self.primary);
            }
        }
    }
}

pub fn encode_padding(seq: u32, bytes: usize, out: &mut Vec<u8>) {
    Writer(out).u8(PADDING).u32(seq);
    out.resize(out.len().max(bytes), 0);
}

pub fn encode_pointer(seq: u32, x: i32, y: i32, out: &mut Vec<u8>) {
    Writer(out).u8(POINTER).u32(seq).i32(x).i32(y);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerDatagram<'a> {
    Video(VideoShard<'a>),
    Audio(AudioPacket<'a>),
    Padding { seq: u32 },
    Pointer { seq: u32, x: i32, y: i32 },
}

impl<'a> ServerDatagram<'a> {
    #[must_use]
    pub fn seq(&self) -> u32 {
        match self {
            ServerDatagram::Video(shard) => shard.seq,
            ServerDatagram::Audio(packet) => packet.seq,
            ServerDatagram::Padding { seq } | ServerDatagram::Pointer { seq, .. } => *seq,
        }
    }

    pub fn decode(bytes: &'a [u8]) -> Result<ServerDatagram<'a>, DecodeError> {
        let (kind, body) = bytes.split_first().ok_or(DecodeError::Empty)?;
        let mut reader = Reader::new(*kind, body);
        let seq = reader.u32()?;
        let datagram = match *kind {
            VIDEO => {
                let stream = reader.u16()?;
                let frame = reader.u32()?;
                let reference = reader.u32()?;
                let flags = FrameFlags::from_bits(reader.u8()?);
                let capture_us = reader.u32()?;
                let frame_bytes = reader.u32()?;
                let block_offset = reader.u32()?;
                let block = reader.u16()?;
                let blocks = reader.u16()?;
                let data_shards = reader.u8()?;
                let parity_shards = reader.u8()?;
                let shard = reader.u8()?;
                reader.u8()?;
                let length = usize::from(reader.u16()?);
                let payload = reader.raw(length)?;
                let shards = u16::from(data_shards) + u16::from(parity_shards);
                if data_shards == 0 || shards > 255 || u16::from(shard) >= shards {
                    return Err(DecodeError::InvalidValue {
                        field: "shard",
                        value: u64::from(shard),
                    });
                }
                if blocks == 0 || block >= blocks || block_offset >= frame_bytes.max(1) {
                    return Err(DecodeError::InvalidValue {
                        field: "block",
                        value: u64::from(block),
                    });
                }
                ServerDatagram::Video(VideoShard {
                    seq,
                    stream,
                    frame,
                    reference,
                    flags,
                    capture_us,
                    frame_bytes,
                    block_offset,
                    block,
                    blocks,
                    data_shards,
                    parity_shards,
                    shard,
                    payload,
                })
            }
            AUDIO => {
                let timestamp = reader.u32()?;
                let flags = reader.u8()?;
                let (primary, previous) = match flags & REDUNDANT != 0 {
                    true => {
                        let length = usize::from(reader.u16()?);
                        let primary = reader.raw(length)?;
                        (primary, Some(reader.rest()))
                    }
                    false => (reader.rest(), None),
                };
                ServerDatagram::Audio(AudioPacket {
                    seq,
                    timestamp,
                    primary,
                    previous,
                })
            }
            PADDING => {
                reader.rest();
                ServerDatagram::Padding { seq }
            }
            POINTER => ServerDatagram::Pointer {
                seq,
                x: reader.i32()?,
                y: reader.i32()?,
            },
            other => return Err(DecodeError::UnknownType(other)),
        };
        reader.finish()?;
        Ok(datagram)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Motion {
    pub seq: u16,
    pub at: Position,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arrival {
    pub seq: u32,
    pub at_us: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Feedback {
    pub base_seq: u32,
    pub count: u16,
    pub arrivals: Vec<Arrival>,
    pub stream: u16,
    pub good: Option<u32>,
    pub lost: Vec<u32>,
}

impl Feedback {
    #[must_use]
    pub fn bytes_for(count: usize, received: usize, lost: usize) -> usize {
        1 + 4 + 2 + 4 + count.div_ceil(8) + 2 * received.saturating_sub(1) + 2 + 4 + 1 + 4 * lost
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        let count = usize::from(self.count);
        let first = self.arrivals.first().map_or(0, |arrival| arrival.at_us);
        let mut writer = Writer(out);
        writer
            .u8(FEEDBACK)
            .u32(self.base_seq)
            .u16(self.count)
            .u32(first);
        let mut bitmap = vec![0_u8; count.div_ceil(8)];
        for arrival in &self.arrivals {
            let index = arrival.seq.wrapping_sub(self.base_seq) as usize;
            if index < count {
                bitmap[index / 8] |= 1 << (index % 8);
            }
        }
        writer.raw(&bitmap);
        let mut previous = first;
        for arrival in self.arrivals.iter().skip(1) {
            let gap = arrival.at_us.wrapping_sub(previous) as i32;
            let units = (i64::from(gap) + i64::from(DELTA_UNIT_US / 2))
                .div_euclid(i64::from(DELTA_UNIT_US))
                .clamp(i64::from(i16::MIN), i64::from(i16::MAX));
            writer.i16(units as i16);
            previous = previous.wrapping_add((units * i64::from(DELTA_UNIT_US)) as u32);
        }
        let lost = &self.lost[..self.lost.len().min(usize::from(u8::MAX))];
        writer
            .u16(self.stream)
            .u32(self.good.unwrap_or(NO_FRAME))
            .u8(lost.len() as u8);
        for frame in lost {
            writer.u32(*frame);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientDatagram {
    Motion(Motion),
    Feedback(Feedback),
}

impl ClientDatagram {
    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            ClientDatagram::Motion(motion) => {
                Writer(out)
                    .u8(MOTION)
                    .u16(motion.seq)
                    .u16(motion.at.x)
                    .u16(motion.at.y);
            }
            ClientDatagram::Feedback(feedback) => feedback.encode(out),
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<ClientDatagram, DecodeError> {
        let (kind, body) = bytes.split_first().ok_or(DecodeError::Empty)?;
        let mut reader = Reader::new(*kind, body);
        let datagram = match *kind {
            MOTION => ClientDatagram::Motion(Motion {
                seq: reader.u16()?,
                at: Position {
                    x: reader.u16()?,
                    y: reader.u16()?,
                },
            }),
            FEEDBACK => ClientDatagram::Feedback(read_feedback(&mut reader)?),
            other => return Err(DecodeError::UnknownType(other)),
        };
        reader.finish()?;
        Ok(datagram)
    }
}

fn read_feedback(reader: &mut Reader<'_>) -> Result<Feedback, DecodeError> {
    let base_seq = reader.u32()?;
    let count = reader.u16()?;
    let first = reader.u32()?;
    let bitmap = reader.raw(usize::from(count).div_ceil(8))?;
    let mut arrivals = Vec::new();
    let mut at = first;
    for index in 0..usize::from(count) {
        if bitmap[index / 8] & (1 << (index % 8)) == 0 {
            continue;
        }
        if !arrivals.is_empty() {
            let units = i32::from(reader.i16()?);
            at = at.wrapping_add((units * DELTA_UNIT_US as i32) as u32);
        }
        arrivals.push(Arrival {
            seq: base_seq.wrapping_add(index as u32),
            at_us: at,
        });
    }
    let stream = reader.u16()?;
    let good = match reader.u32()? {
        NO_FRAME => None,
        frame => Some(frame),
    };
    let lost_count = reader.u8()?;
    let lost = (0..lost_count)
        .map(|_| reader.u32())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Feedback {
        base_seq,
        count,
        arrivals,
        stream,
        good,
        lost,
    })
}
