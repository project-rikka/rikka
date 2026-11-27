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

const LONGEST_MATCH: usize = 258;
const LITERAL_BURST: usize = 3;
const SLACK: usize = 16;
const FIRST_GROWTH: usize = 1 << 15;
const REPORT_SPAN: usize = 1 << 15;

struct Output<'o> {
    bytes: &'o mut Vec<u8>,
    start: usize,
    at: usize,
    limit: usize,
    reported: usize,
    report: &'o mut dyn FnMut(&[u8]),
}

impl Output<'_> {
    #[inline]
    fn produced(&self) -> usize {
        self.at - self.start
    }

    #[inline]
    fn room(&self) -> usize {
        self.limit - self.produced()
    }

    fn too_large(&self) -> Error {
        Error::TooLarge { limit: self.limit }
    }

    #[inline]
    fn ensure(&mut self, more: usize) {
        let wanted = self.at + (more + SLACK).min(self.room());
        if wanted > self.bytes.len() {
            self.grow(wanted);
        }
    }

    #[cold]
    fn grow(&mut self, wanted: usize) {
        let ceiling = self.start.saturating_add(self.limit);
        let doubled = self.bytes.len().saturating_mul(2).max(FIRST_GROWTH);
        let size = doubled.max(self.bytes.capacity()).max(wanted).min(ceiling);
        self.bytes.resize(size, 0);
    }

    #[inline]
    fn repeat(&mut self, distance: usize, length: usize) {
        let end = self.at + length;
        let mut from = self.at - distance;
        if distance >= SLACK && end + SLACK <= self.bytes.len() {
            while self.at < end {
                self.bytes.copy_within(from..from + SLACK, self.at);
                from += SLACK;
                self.at += SLACK;
            }
        } else if distance == 1 {
            let byte = self.bytes[from];
            self.bytes[self.at..end].fill(byte);
        } else {
            for at in self.at..end {
                self.bytes[at] = self.bytes[from];
                from += 1;
            }
        }
        self.at = end;
    }

    #[inline]
    fn report_if_due(&mut self) {
        if self.at - self.reported >= REPORT_SPAN {
            self.report();
        }
    }

    fn report(&mut self) {
        self.reported = self.at;
        (self.report)(&self.bytes[self.start..self.at]);
    }

    fn finish(self) {
        self.bytes.truncate(self.at);
    }
}

impl Inflater {
    #[must_use]
    pub fn new() -> Inflater {
        Inflater::default()
    }

    pub fn inflate(&mut self, data: &[u8], out: &mut Vec<u8>, limit: usize) -> Result<usize> {
        self.inflate_reporting(data, out, limit, &mut |_| {})
    }

    pub(crate) fn inflate_reporting(
        &mut self,
        data: &[u8],
        out: &mut Vec<u8>,
        limit: usize,
        report: &mut dyn FnMut(&[u8]),
    ) -> Result<usize> {
        let mut out = Output {
            start: out.len(),
            at: out.len(),
            bytes: out,
            limit,
            reported: 0,
            report,
        };
        out.reported = out.start;
        let inflated = self.blocks(data, &mut out);
        if inflated.is_ok() {
            out.report();
        }
        out.finish();
        inflated
    }

    fn blocks(&mut self, data: &[u8], out: &mut Output<'_>) -> Result<usize> {
        let mut bits = BitReader::new(data);
        loop {
            let header = bits.read(3);
            let last = header & 1 == 1;
            match header >> 1 {
                0 => stored(&mut bits, out)?,
                1 => {
                    self.prepare_fixed();
                    codes(&self.fixed_literals, &self.fixed_distances, &mut bits, out)?;
                }
                2 => {
                    self.read_dynamic(&mut bits)?;
                    codes(&self.literals, &self.distances, &mut bits, out)?;
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
    out.ensure(len);
    out.bytes[out.at..out.at + len].copy_from_slice(bytes);
    out.at += len;
    Ok(())
}

fn codes(
    literals: &DecodingTable,
    distances: &DecodingTable,
    bits: &mut BitReader<'_>,
    out: &mut Output<'_>,
) -> Result<()> {
    let mut reader = bits.clone();
    let mut writer = Output {
        bytes: &mut *out.bytes,
        start: out.start,
        at: out.at,
        limit: out.limit,
        reported: out.reported,
        report: &mut *out.report,
    };
    let decoded = decode_codes(literals, distances, &mut reader, &mut writer);
    out.at = writer.at;
    out.reported = writer.reported;
    *bits = reader;
    decoded
}

#[inline(always)]
fn decode_codes(
    literals: &DecodingTable,
    distances: &DecodingTable,
    bits: &mut BitReader<'_>,
    out: &mut Output<'_>,
) -> Result<()> {
    loop {
        out.report_if_due();
        bits.refill();
        out.ensure(LONGEST_MATCH);
        let mut symbol = literals.decode(bits).ok_or_else(|| malformed(bits))?;
        let mut burst = LITERAL_BURST;
        while symbol < END_OF_BLOCK {
            if out.room() == 0 {
                return Err(match bits.is_overrun() {
                    true => Error::Truncated,
                    false => out.too_large(),
                });
            }
            out.bytes[out.at] = symbol as u8;
            out.at += 1;
            burst -= 1;
            if burst == 0 {
                break;
            }
            symbol = literals.decode(bits).ok_or_else(|| malformed(bits))?;
        }
        if burst == 0 {
            continue;
        }
        if symbol == END_OF_BLOCK {
            return Ok(());
        }
        bits.refill();
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
        out.repeat(distance, length);
    }
}
