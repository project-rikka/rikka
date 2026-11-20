use crate::deflate::bit_writer::BitWriter;
use crate::deflate::prefix_code::PrefixCode;
use crate::tables::{
    CODE_LENGTH_CODES, CODE_LENGTH_ORDER, DISTANCE_BASE, DISTANCE_CODES, DISTANCE_EXTRA,
    END_OF_BLOCK, FIRST_LENGTH, FIXED_DISTANCE_LENGTHS, LENGTH_BASE, LENGTH_EXTRA, LITERAL_SYMBOLS,
    MAX_CODE_BITS, MIN_MATCH, WINDOW, fixed_literal_lengths,
};

const MATCH_FLAG: u32 = 1 << 31;
const LENGTH_SHIFT: u32 = 16;
const DISTANCE_MASK: u32 = 0x7FFF;
const CODE_LENGTH_LIMIT: usize = 7;
const STORED: u32 = 0;
const FIXED: u32 = 1;
const DYNAMIC: u32 = 2;
const STORED_LIMIT: usize = 65535;
const STORED_OVERHEAD_BITS: u64 = 3 + 7 + 32;
const REPEAT_PREVIOUS: u8 = 16;
const REPEAT_ZERO_SHORT: u8 = 17;
const REPEAT_ZERO_LONG: u8 = 18;
const MAX_HEADER_LENGTHS: usize = LITERAL_SYMBOLS + DISTANCE_CODES;

static LENGTH_SYMBOLS: [u8; 256] = {
    let mut symbols = [0u8; 256];
    let mut code = 0;
    while code < LENGTH_BASE.len() {
        let first = LENGTH_BASE[code] as usize - MIN_MATCH;
        let last = if code + 1 < LENGTH_BASE.len() {
            LENGTH_BASE[code + 1] as usize - MIN_MATCH
        } else {
            256
        };
        let mut length = first;
        while length < last {
            symbols[length] = code as u8;
            length += 1;
        }
        code += 1;
    }
    symbols
};

static DISTANCE_SYMBOLS: [u8; 512] = {
    let mut symbols = [0u8; 512];
    let mut code = 0;
    let mut offset = 0;
    while offset < WINDOW {
        while code + 1 < DISTANCE_BASE.len() && DISTANCE_BASE[code + 1] as usize - 1 <= offset {
            code += 1;
        }
        if offset < 256 {
            symbols[offset] = code as u8;
            offset += 1;
        } else {
            symbols[256 + (offset >> 7)] = code as u8;
            offset += 128;
        }
    }
    symbols
};

#[inline]
fn distance_symbol(offset: usize) -> usize {
    usize::from(if offset < 256 {
        DISTANCE_SYMBOLS[offset]
    } else {
        DISTANCE_SYMBOLS[256 + (offset >> 7)]
    })
}

#[derive(Debug)]
pub(crate) struct Block {
    tokens: Vec<u32>,
    literals: [u32; LITERAL_SYMBOLS],
    distances: [u32; DISTANCE_CODES],
    extra_bits: u64,
}

impl Block {
    pub(crate) fn new() -> Self {
        Self {
            tokens: Vec::new(),
            literals: [0; LITERAL_SYMBOLS],
            distances: [0; DISTANCE_CODES],
            extra_bits: 0,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.tokens.clear();
        self.literals = [0; LITERAL_SYMBOLS];
        self.distances = [0; DISTANCE_CODES];
        self.extra_bits = 0;
    }

    pub(crate) fn len(&self) -> usize {
        self.tokens.len()
    }

    #[inline]
    pub(crate) fn literal(&mut self, byte: u8) {
        self.tokens.push(u32::from(byte));
        self.literals[usize::from(byte)] += 1;
    }

    #[inline]
    pub(crate) fn repeat(&mut self, length: usize, distance: usize) {
        let length = length - MIN_MATCH;
        let offset = distance - 1;
        self.tokens
            .push(MATCH_FLAG | (length as u32) << LENGTH_SHIFT | offset as u32);
        let length_symbol = usize::from(LENGTH_SYMBOLS[length]);
        let distance_symbol = distance_symbol(offset);
        self.literals[usize::from(FIRST_LENGTH) + length_symbol] += 1;
        self.distances[distance_symbol] += 1;
        self.extra_bits +=
            u64::from(LENGTH_EXTRA[length_symbol]) + u64::from(DISTANCE_EXTRA[distance_symbol]);
    }

    pub(crate) fn write(&mut self, bits: &mut BitWriter<'_>, input: &[u8], last: bool) {
        self.literals[usize::from(END_OF_BLOCK)] += 1;
        let literals = PrefixCode::from_counts(&self.literals, MAX_CODE_BITS);
        let distances = PrefixCode::from_counts(&self.distances, MAX_CODE_BITS);
        let header = Header::new(&literals, &distances);
        let dynamic_bits =
            header.bits + literals.cost(&self.literals) + distances.cost(&self.distances);

        let fixed_literals = PrefixCode::from_lengths(fixed_literal_lengths());
        let fixed_distances = PrefixCode::from_lengths(FIXED_DISTANCE_LENGTHS);
        let fixed_bits =
            fixed_literals.cost(&self.literals) + fixed_distances.cost(&self.distances);

        let stored_blocks = input.len().div_ceil(STORED_LIMIT).max(1) as u64;
        let stored_bits = stored_blocks * STORED_OVERHEAD_BITS + input.len() as u64 * 8;
        let coded_bits = dynamic_bits.min(fixed_bits) + self.extra_bits + 3;

        if stored_bits <= coded_bits {
            stored(bits, input, last);
        } else if fixed_bits <= dynamic_bits {
            bits.put(u32::from(last) | FIXED << 1, 3);
            self.tokens_into(bits, &fixed_literals, &fixed_distances);
        } else {
            bits.put(u32::from(last) | DYNAMIC << 1, 3);
            header.write(bits);
            self.tokens_into(bits, &literals, &distances);
        }
        self.clear();
    }

    fn tokens_into(
        &self,
        bits: &mut BitWriter<'_>,
        literals: &PrefixCode<LITERAL_SYMBOLS>,
        distances: &PrefixCode<DISTANCE_CODES>,
    ) {
        for &token in &self.tokens {
            if token & MATCH_FLAG == 0 {
                let symbol = token as usize;
                bits.put(
                    u32::from(literals.codes[symbol]),
                    u32::from(literals.lengths[symbol]),
                );
                continue;
            }
            let length = ((token >> LENGTH_SHIFT) & 0xFF) as usize;
            let offset = (token & DISTANCE_MASK) as usize;
            let length_symbol = usize::from(LENGTH_SYMBOLS[length]);
            let symbol = usize::from(FIRST_LENGTH) + length_symbol;
            bits.put(
                u32::from(literals.codes[symbol]),
                u32::from(literals.lengths[symbol]),
            );
            bits.put(
                (length + MIN_MATCH - usize::from(LENGTH_BASE[length_symbol])) as u32,
                u32::from(LENGTH_EXTRA[length_symbol]),
            );
            let distance_symbol = distance_symbol(offset);
            bits.put(
                u32::from(distances.codes[distance_symbol]),
                u32::from(distances.lengths[distance_symbol]),
            );
            bits.put(
                (offset + 1 - usize::from(DISTANCE_BASE[distance_symbol])) as u32,
                u32::from(DISTANCE_EXTRA[distance_symbol]),
            );
        }
        let end = usize::from(END_OF_BLOCK);
        bits.put(
            u32::from(literals.codes[end]),
            u32::from(literals.lengths[end]),
        );
    }
}

pub(crate) fn stored(bits: &mut BitWriter<'_>, input: &[u8], last: bool) {
    let mut pieces = input.chunks(STORED_LIMIT).peekable();
    if pieces.peek().is_none() {
        bits.put(u32::from(last) | STORED << 1, 3);
        bits.bytes(&[0, 0, 0xFF, 0xFF]);
        return;
    }
    while let Some(piece) = pieces.next() {
        let last = last && pieces.peek().is_none();
        bits.put(u32::from(last) | STORED << 1, 3);
        let length = piece.len() as u16;
        bits.bytes(&length.to_le_bytes());
        bits.bytes(&(!length).to_le_bytes());
        bits.bytes(piece);
    }
}

#[derive(Clone, Copy, Debug)]
struct Run {
    symbol: u8,
    extra: u8,
}

struct Header {
    literal_count: usize,
    distance_count: usize,
    code_length_count: usize,
    runs: [Run; MAX_HEADER_LENGTHS],
    run_count: usize,
    code_lengths: PrefixCode<CODE_LENGTH_CODES>,
    bits: u64,
}

impl Header {
    fn new(literals: &PrefixCode<LITERAL_SYMBOLS>, distances: &PrefixCode<DISTANCE_CODES>) -> Self {
        let literal_count = used(&literals.lengths).max(257);
        let distance_count = used(&distances.lengths).max(1);
        let mut lengths = [0u8; MAX_HEADER_LENGTHS];
        lengths[..literal_count].copy_from_slice(&literals.lengths[..literal_count]);
        lengths[literal_count..literal_count + distance_count]
            .copy_from_slice(&distances.lengths[..distance_count]);
        let lengths = &lengths[..literal_count + distance_count];

        let mut runs = [Run {
            symbol: 0,
            extra: 0,
        }; MAX_HEADER_LENGTHS];
        let mut run_count = 0;
        let mut counts = [0u32; CODE_LENGTH_CODES];
        let mut emit = |symbol: u8, extra: usize| {
            runs[run_count] = Run {
                symbol,
                extra: extra as u8,
            };
            run_count += 1;
            counts[usize::from(symbol)] += 1;
        };
        let mut index = 0;
        while index < lengths.len() {
            let value = lengths[index];
            let run = lengths[index..]
                .iter()
                .take_while(|&&length| length == value)
                .count();
            index += run;
            let mut left = run;
            if value == 0 {
                while left >= 11 {
                    let taken = left.min(138);
                    emit(REPEAT_ZERO_LONG, taken - 11);
                    left -= taken;
                }
                if left >= 3 {
                    emit(REPEAT_ZERO_SHORT, left - 3);
                    left = 0;
                }
            } else {
                emit(value, 0);
                left -= 1;
                while left >= 3 {
                    let taken = left.min(6);
                    emit(REPEAT_PREVIOUS, taken - 3);
                    left -= taken;
                }
            }
            for _ in 0..left {
                emit(value, 0);
            }
        }

        let code_lengths = PrefixCode::from_counts(&counts, CODE_LENGTH_LIMIT);
        let code_length_count = CODE_LENGTH_ORDER
            .iter()
            .rposition(|&symbol| code_lengths.lengths[symbol] != 0)
            .map_or(4, |last| (last + 1).max(4));
        let extra: u64 = runs[..run_count]
            .iter()
            .map(|run| u64::from(extra_bits(run.symbol)))
            .sum();
        let bits = 14 + 3 * code_length_count as u64 + code_lengths.cost(&counts) + extra;
        Self {
            literal_count,
            distance_count,
            code_length_count,
            runs,
            run_count,
            code_lengths,
            bits,
        }
    }

    fn write(&self, bits: &mut BitWriter<'_>) {
        bits.put((self.literal_count - 257) as u32, 5);
        bits.put((self.distance_count - 1) as u32, 5);
        bits.put((self.code_length_count - 4) as u32, 4);
        for &symbol in &CODE_LENGTH_ORDER[..self.code_length_count] {
            bits.put(u32::from(self.code_lengths.lengths[symbol]), 3);
        }
        for run in &self.runs[..self.run_count] {
            let symbol = usize::from(run.symbol);
            bits.put(
                u32::from(self.code_lengths.codes[symbol]),
                u32::from(self.code_lengths.lengths[symbol]),
            );
            bits.put(u32::from(run.extra), extra_bits(run.symbol));
        }
    }
}

fn used(lengths: &[u8]) -> usize {
    lengths
        .iter()
        .rposition(|&length| length != 0)
        .map_or(0, |last| last + 1)
}

const fn extra_bits(symbol: u8) -> u32 {
    match symbol {
        REPEAT_PREVIOUS => 2,
        REPEAT_ZERO_SHORT => 3,
        REPEAT_ZERO_LONG => 7,
        _ => 0,
    }
}
