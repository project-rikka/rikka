use crate::error::{Error, Result};
use crate::format::Format;

const FORMAT: Format = Format::Avif;
const SEQUENCE_HEADER: u8 = 1;
const TEMPORAL_DELIMITER: u8 = 2;
const PADDING: u8 = 15;
const CONFIG_MARKER: u8 = 0x80;
const CONFIG_VERSION: u8 = 1;
const CONFIG_HEADER: usize = 4;
const SELECT_SCREEN_CONTENT_TOOLS: u32 = 2;
const PRIMARIES_BT709: u32 = 1;
const TRANSFER_SRGB: u32 = 13;
const MATRIX_IDENTITY: u32 = 0;
const PROFILE_PROFESSIONAL: u32 = 2;

#[derive(Debug)]
pub(crate) struct Still {
    pub(crate) data: Vec<u8>,
    pub(crate) config: Vec<u8>,
}

impl Still {
    pub(crate) fn from_encoder(packet: &[u8], extradata: &[u8]) -> Result<Still> {
        let mut data = Vec::with_capacity(packet.len());
        let mut sequence = None;
        for obu in Obus::new(packet) {
            let obu = obu?;
            match obu.kind {
                TEMPORAL_DELIMITER | PADDING => {}
                kind => {
                    if kind == SEQUENCE_HEADER && sequence.is_none() {
                        sequence = Some(obu);
                    }
                    data.extend_from_slice(obu.whole);
                }
            }
        }
        let sequence = match sequence {
            Some(sequence) => sequence,
            None => {
                let configured = configured_obus(extradata);
                let header = Obus::new(configured)
                    .find(|obu| obu.as_ref().is_ok_and(|obu| obu.kind == SEQUENCE_HEADER))
                    .ok_or_else(|| {
                        Error::malformed(FORMAT, "AV1 stream without a sequence header")
                    })??;
                let mut whole = header.whole.to_vec();
                whole.extend_from_slice(&data);
                data = whole;
                header
            }
        };
        let header = SequenceHeader::parse(sequence.payload)?;
        let config = header.config_record();
        Ok(Still { data, config })
    }
}

fn configured_obus(extradata: &[u8]) -> &[u8] {
    match extradata.first() {
        Some(first) if first & CONFIG_MARKER != 0 && extradata.len() >= CONFIG_HEADER => {
            &extradata[CONFIG_HEADER..]
        }
        _ => extradata,
    }
}

#[derive(Clone, Copy, Debug)]
struct Obu<'d> {
    kind: u8,
    whole: &'d [u8],
    payload: &'d [u8],
}

struct Obus<'d> {
    data: &'d [u8],
}

impl<'d> Obus<'d> {
    fn new(data: &'d [u8]) -> Obus<'d> {
        Obus { data }
    }

    fn next_obu(&mut self) -> Result<Obu<'d>> {
        let truncated = || Error::malformed(FORMAT, "AV1 OBU");
        let header = *self.data.first().ok_or_else(truncated)?;
        let kind = (header >> 3) & 0x0F;
        let extended = header & 0x04 != 0;
        let sized = header & 0x02 != 0;
        let mut at = 1 + usize::from(extended);
        if self.data.len() < at {
            return Err(truncated());
        }
        let size = match sized {
            true => {
                let (size, read) = leb128(&self.data[at..]).ok_or_else(truncated)?;
                at += read;
                usize::try_from(size).map_err(|_| truncated())?
            }
            false => self.data.len() - at,
        };
        let end = at.checked_add(size).filter(|end| *end <= self.data.len());
        let end = end.ok_or_else(truncated)?;
        let obu = Obu {
            kind,
            whole: &self.data[..end],
            payload: &self.data[at..end],
        };
        self.data = &self.data[end..];
        Ok(obu)
    }
}

impl<'d> Iterator for Obus<'d> {
    type Item = Result<Obu<'d>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }
        let obu = self.next_obu();
        if obu.is_err() {
            self.data = &[];
        }
        Some(obu)
    }
}

fn leb128(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0_u64;
    for (index, byte) in bytes.iter().take(8).enumerate() {
        value |= u64::from(byte & 0x7F) << (index * 7);
        if byte & 0x80 == 0 {
            return Some((value, index + 1));
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SequenceHeader {
    profile: u8,
    level: u8,
    tier: u8,
    high_bitdepth: bool,
    twelve_bit: bool,
    monochrome: bool,
    subsampling_x: bool,
    subsampling_y: bool,
    chroma_sample_position: u8,
}

impl SequenceHeader {
    fn parse(payload: &[u8]) -> Result<SequenceHeader> {
        let mut bits = Bits::new(payload);
        let profile = bits.read(3)?;
        bits.skip(1)?;
        let reduced = bits.flag()?;
        let (level, tier) = match reduced {
            true => (bits.read(5)?, 0),
            false => operating_points(&mut bits)?,
        };
        let width_bits = bits.read(4)? + 1;
        let height_bits = bits.read(4)? + 1;
        bits.skip(width_bits + height_bits)?;
        if !reduced && bits.flag()? {
            bits.skip(4 + 3)?;
        }
        bits.skip(3)?;
        if !reduced {
            bits.skip(4)?;
            let order_hint = bits.flag()?;
            if order_hint {
                bits.skip(2)?;
            }
            let screen_content = match bits.flag()? {
                true => SELECT_SCREEN_CONTENT_TOOLS,
                false => bits.read(1)?,
            };
            if screen_content > 0 && !bits.flag()? {
                bits.skip(1)?;
            }
            if order_hint {
                bits.skip(3)?;
            }
        }
        bits.skip(3)?;
        color_config(&mut bits, profile, level, tier)
    }

    fn config_record(self) -> Vec<u8> {
        let bit = |flag: bool, shift: u8| u8::from(flag) << shift;
        vec![
            CONFIG_MARKER | CONFIG_VERSION,
            (self.profile << 5) | self.level,
            (self.tier << 7)
                | bit(self.high_bitdepth, 6)
                | bit(self.twelve_bit, 5)
                | bit(self.monochrome, 4)
                | bit(self.subsampling_x, 3)
                | bit(self.subsampling_y, 2)
                | self.chroma_sample_position,
            0,
        ]
    }
}

fn operating_points(bits: &mut Bits<'_>) -> Result<(u32, u32)> {
    let timing = bits.flag()?;
    let mut decoder_model = false;
    let mut delay_length = 0;
    if timing {
        bits.skip(64)?;
        if bits.flag()? {
            bits.uvlc()?;
        }
        decoder_model = bits.flag()?;
        if decoder_model {
            delay_length = bits.read(5)? + 1;
            bits.skip(32 + 5 + 5)?;
        }
    }
    let display_delay = bits.flag()?;
    let points = bits.read(5)? + 1;
    let mut first = None;
    for _ in 0..points {
        bits.skip(12)?;
        let level = bits.read(5)?;
        let tier = match level > 7 {
            true => bits.read(1)?,
            false => 0,
        };
        if decoder_model && bits.flag()? {
            bits.skip(2 * delay_length + 1)?;
        }
        if display_delay && bits.flag()? {
            bits.skip(4)?;
        }
        first.get_or_insert((level, tier));
    }
    first.ok_or_else(|| Error::malformed(FORMAT, "AV1 operating points"))
}

fn color_config(
    bits: &mut Bits<'_>,
    profile: u32,
    level: u32,
    tier: u32,
) -> Result<SequenceHeader> {
    let high_bitdepth = bits.flag()?;
    let twelve_bit = profile == PROFILE_PROFESSIONAL && high_bitdepth && bits.flag()?;
    let monochrome = profile != 1 && bits.flag()?;
    let (primaries, transfer, matrix) = match bits.flag()? {
        true => (bits.read(8)?, bits.read(8)?, bits.read(8)?),
        false => (2, 2, 2),
    };
    let header = |subsampling_x, subsampling_y, chroma_sample_position| SequenceHeader {
        profile: profile as u8,
        level: level as u8,
        tier: tier as u8,
        high_bitdepth,
        twelve_bit,
        monochrome,
        subsampling_x,
        subsampling_y,
        chroma_sample_position,
    };
    if monochrome {
        bits.skip(1)?;
        return Ok(header(true, true, 0));
    }
    if primaries == PRIMARIES_BT709 && transfer == TRANSFER_SRGB && matrix == MATRIX_IDENTITY {
        return Ok(header(false, false, 0));
    }
    bits.skip(1)?;
    let (subsampling_x, subsampling_y) = match profile {
        0 => (true, true),
        1 => (false, false),
        _ if twelve_bit => {
            let x = bits.flag()?;
            (x, x && bits.flag()?)
        }
        _ => (true, false),
    };
    let position = match subsampling_x && subsampling_y {
        true => bits.read(2)? as u8,
        false => 0,
    };
    Ok(header(subsampling_x, subsampling_y, position))
}

struct Bits<'d> {
    data: &'d [u8],
    at: usize,
}

impl<'d> Bits<'d> {
    fn new(data: &'d [u8]) -> Bits<'d> {
        Bits { data, at: 0 }
    }

    fn flag(&mut self) -> Result<bool> {
        let byte = *self
            .data
            .get(self.at / 8)
            .ok_or_else(|| Error::malformed(FORMAT, "AV1 sequence header"))?;
        let set = (byte >> (7 - self.at % 8)) & 1 == 1;
        self.at += 1;
        Ok(set)
    }

    fn read(&mut self, count: u32) -> Result<u32> {
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1) | u32::from(self.flag()?);
        }
        Ok(value)
    }

    fn skip(&mut self, count: u32) -> Result<()> {
        let end = self.at + count as usize;
        if end > self.data.len() * 8 {
            return Err(Error::malformed(FORMAT, "AV1 sequence header"));
        }
        self.at = end;
        Ok(())
    }

    fn uvlc(&mut self) -> Result<()> {
        let mut zeros = 0;
        while !self.flag()? {
            zeros += 1;
            if zeros >= 32 {
                return Err(Error::malformed(FORMAT, "AV1 sequence header"));
            }
        }
        self.skip(zeros)
    }
}
