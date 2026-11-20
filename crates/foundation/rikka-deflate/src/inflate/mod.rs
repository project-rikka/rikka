mod bit_reader;
mod decoding_table;

use crate::error::{Error, Result};
use crate::tables::{
    CODE_LENGTH_ORDER, DISTANCE_BASE, DISTANCE_CODES, DISTANCE_EXTRA, END_OF_BLOCK,
    FIXED_DISTANCE_LENGTHS, LENGTH_BASE, LENGTH_EXTRA, LITERAL_CODES, fixed_literal_lengths,
};
use bit_reader::BitReader;
use decoding_table::DecodingTable;

#[derive(Debug, Default)]
pub struct Inflater {
    literals: DecodingTable,
    distances: DecodingTable,
    code_lengths: DecodingTable,
    fixed_literals: DecodingTable,
    fixed_distances: DecodingTable,
    fixed_ready: bool,
}

struct Output<'o> {
    bytes: &'o mut Vec<u8>,
    start: usize,
    limit: usize,
}

impl Output<'_> {
    #[inline]
    fn produced(&self) -> usize {
        self.bytes.len() - self.start
    }

    #[inline]
    fn room(&self) -> usize {
        self.limit - self.produced()
    }

    fn too_large(&self) -> Error {
        Error::TooLarge { limit: self.limit }
    }
}

impl Inflater {
    #[must_use]
    pub fn new() -> Inflater {
        Inflater::default()
    }

    pub fn inflate(&mut self, data: &[u8], out: &mut Vec<u8>, limit: usize) -> Result<usize> {
        let mut bits = BitReader::new(data);
        let mut out = Output {
            start: out.len(),
            bytes: out,
            limit,
        };
        loop {
            let header = bits.read(3);
            let last = header & 1 == 1;
            match header >> 1 {
                0 => stored(&mut bits, &mut out)?,
                1 => {
                    self.prepare_fixed();
                    codes(
                        &self.fixed_literals,
                        &self.fixed_distances,
                        &mut bits,
                        &mut out,
                    )?;
                }
                2 => {
                    self.read_dynamic(&mut bits)?;
                    codes(&self.literals, &self.distances, &mut bits, &mut out)?;
                }
                _ => return Err(malformed(&bits)),
            }
            if bits.is_overrun() {
                return Err(Error::Truncated);
            }
            if last {
                return bits.byte_position_rounded_up().ok_or(Error::Truncated);
            }
        }
    }

    fn prepare_fixed(&mut self) {
        if self.fixed_ready {
            return;
        }
        self.fixed_literals.build(&fixed_literal_lengths());
        self.fixed_distances.build(&FIXED_DISTANCE_LENGTHS);
        self.fixed_ready = true;
    }

    fn read_dynamic(&mut self, bits: &mut BitReader<'_>) -> Result<()> {
        let literal_count = bits.read(5) as usize + 257;
        let distance_count = bits.read(5) as usize + 1;
        let code_length_count = bits.read(4) as usize + 4;
        if literal_count > LITERAL_CODES || distance_count > DISTANCE_CODES {
            return Err(malformed(bits));
        }
        let mut code_lengths = [0u8; 19];
        for &symbol in &CODE_LENGTH_ORDER[..code_length_count] {
            code_lengths[symbol] = bits.read(3) as u8;
        }
        if !self.code_lengths.build(&code_lengths) {
            return Err(malformed(bits));
        }
        let total = literal_count + distance_count;
        let mut lengths = [0u8; LITERAL_CODES + DISTANCE_CODES];
        let mut index = 0;
        while index < total {
            bits.refill();
            let symbol = self
                .code_lengths
                .decode(bits)
                .ok_or_else(|| malformed(bits))?;
            let (value, repeat) = match symbol {
                0..=15 => (symbol as u8, 1),
                16 => {
                    let previous = index.checked_sub(1).ok_or(Error::Malformed)?;
                    (lengths[previous], 3 + bits.take(2) as usize)
                }
                17 => (0, 3 + bits.take(3) as usize),
                18 => (0, 11 + bits.take(7) as usize),
                _ => return Err(Error::Malformed),
            };
            let end = index + repeat;
            if end > total {
                return Err(malformed(bits));
            }
            lengths[index..end].fill(value);
            index = end;
        }
        if bits.is_overrun() {
            return Err(Error::Truncated);
        }
        if lengths[usize::from(END_OF_BLOCK)] == 0 {
            return Err(Error::Malformed);
        }
        let built = self.literals.build(&lengths[..literal_count])
            && self.distances.build(&lengths[literal_count..total]);
        built.then_some(()).ok_or(Error::Malformed)
    }
}

fn malformed(bits: &BitReader<'_>) -> Error {
    if bits.is_overrun() {
        Error::Truncated
    } else {
        Error::Malformed
    }
}

fn stored(bits: &mut BitReader<'_>, out: &mut Output<'_>) -> Result<()> {
    bits.align_to_byte();
    let len = bits.read(16) as usize;
    let complement = bits.read(16) as usize;
    if bits.is_overrun() {
        return Err(Error::Truncated);
    }
    if len != !complement & 0xFFFF {
        return Err(Error::Malformed);
    }
    if len > out.room() {
        return Err(out.too_large());
    }
    let start = bits.byte_position().ok_or(Error::Truncated)?;
    let bytes = bits.skip_bytes_from(start, len).ok_or(Error::Truncated)?;
    out.bytes.extend_from_slice(bytes);
    Ok(())
}

fn codes(
    literals: &DecodingTable,
    distances: &DecodingTable,
    bits: &mut BitReader<'_>,
    out: &mut Output<'_>,
) -> Result<()> {
    loop {
        bits.refill();
        let symbol = literals.decode(bits).ok_or_else(|| malformed(bits))?;
        if symbol < END_OF_BLOCK {
            if out.room() == 0 {
                return Err(match bits.is_overrun() {
                    true => Error::Truncated,
                    false => out.too_large(),
                });
            }
            out.bytes.push(symbol as u8);
            continue;
        }
        if symbol == END_OF_BLOCK {
            return Ok(());
        }
        let index = usize::from(symbol - 257);
        let length = usize::from(*LENGTH_BASE.get(index).ok_or(Error::Malformed)?)
            + bits.take(u32::from(LENGTH_EXTRA[index])) as usize;
        let code = usize::from(distances.decode(bits).ok_or_else(|| malformed(bits))?);
        let distance = usize::from(*DISTANCE_BASE.get(code).ok_or(Error::Malformed)?)
            + bits.take(u32::from(DISTANCE_EXTRA[code])) as usize;
        if bits.is_overrun() {
            return Err(Error::Truncated);
        }
        if distance > out.produced() {
            return Err(Error::Malformed);
        }
        if length > out.room() {
            return Err(out.too_large());
        }
        repeat(out.bytes, distance, length);
    }
}

#[inline]
fn repeat(out: &mut Vec<u8>, distance: usize, length: usize) {
    let start = out.len() - distance;
    if distance >= length {
        out.extend_from_within(start..start + length);
        return;
    }
    if distance == 1 {
        let byte = out[start];
        out.resize(out.len() + length, byte);
        return;
    }
    let mut available = distance;
    let mut remaining = length;
    while remaining > 0 {
        let run = available.min(remaining);
        out.extend_from_within(start..start + run);
        available += run;
        remaining -= run;
    }
}
