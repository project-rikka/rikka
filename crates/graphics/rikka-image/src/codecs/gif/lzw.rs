const MAX_BITS: u32 = 12;
const TABLE: usize = 1 << MAX_BITS;
const SLOTS: usize = TABLE * 2;
const SLOT_MASK: usize = SLOTS - 1;
const EMPTY: u32 = u32::MAX;
const HASH_MULTIPLIER: u32 = 0x9E37_79B1;
const HASH_SHIFT: u32 = 32 - (MAX_BITS + 1);
const LONGEST_BLOCK: usize = 255;

pub(crate) const MAX_MIN_BITS: u8 = 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Progress {
    Hungry,
    Finished,
    Malformed,
}

#[derive(Debug)]
pub(crate) struct Decoder {
    min_bits: u32,
    code_bits: u32,
    next: usize,
    previous: Option<usize>,
    buffer: u32,
    buffered: u32,
    finished: bool,
    prefix: Vec<u16>,
    suffix: Vec<u8>,
    first: Vec<u8>,
    length: Vec<u16>,
}

impl Decoder {
    pub(crate) fn new() -> Self {
        Self {
            min_bits: 2,
            code_bits: 3,
            next: 6,
            previous: None,
            buffer: 0,
            buffered: 0,
            finished: false,
            prefix: vec![0; TABLE],
            suffix: vec![0; TABLE],
            first: vec![0; TABLE],
            length: vec![0; TABLE],
        }
    }

    pub(crate) fn start(&mut self, min_bits: u8) {
        self.min_bits = u32::from(min_bits);
        self.buffer = 0;
        self.buffered = 0;
        self.finished = false;
        let literals = 1usize << self.min_bits;
        for code in 0..literals {
            self.suffix[code] = code as u8;
            self.first[code] = code as u8;
            self.length[code] = 1;
        }
        self.clear();
    }

    fn clear(&mut self) {
        self.code_bits = self.min_bits + 1;
        self.next = (1 << self.min_bits) + 2;
        self.previous = None;
    }

    pub(crate) fn feed(&mut self, data: &[u8], out: &mut Vec<u8>, limit: usize) -> Progress {
        let clear = 1usize << self.min_bits;
        let end = clear + 1;
        let mut data = data.iter();
        loop {
            if self.finished || out.len() >= limit {
                self.finished = true;
                return Progress::Finished;
            }
            while self.buffered < self.code_bits {
                let Some(&byte) = data.next() else {
                    return Progress::Hungry;
                };
                self.buffer |= u32::from(byte) << self.buffered;
                self.buffered += 8;
            }
            let code = (self.buffer & ((1 << self.code_bits) - 1)) as usize;
            self.buffer >>= self.code_bits;
            self.buffered -= self.code_bits;
            if code == clear {
                self.clear();
                continue;
            }
            if code == end {
                self.finished = true;
                return Progress::Finished;
            }
            let Some(previous) = self.previous else {
                if code > clear {
                    return Progress::Malformed;
                }
                out.push(code as u8);
                self.previous = Some(code);
                if self.next >= 1 << self.code_bits && self.code_bits < MAX_BITS {
                    self.code_bits += 1;
                }
                continue;
            };
            let repeats_first = code == self.next;
            if code > self.next || (repeats_first && self.next >= TABLE) {
                return Progress::Malformed;
            }
            let leading = match repeats_first {
                true => self.first[previous],
                false => self.first[code],
            };
            if self.next < TABLE {
                self.prefix[self.next] = previous as u16;
                self.suffix[self.next] = leading;
                self.first[self.next] = self.first[previous];
                self.length[self.next] = self.length[previous].saturating_add(1);
                self.next += 1;
                if self.next == 1 << self.code_bits && self.code_bits < MAX_BITS {
                    self.code_bits += 1;
                }
            }
            let length = usize::from(self.length[code]).min(limit - out.len());
            let full = usize::from(self.length[code]);
            let start = out.len();
            out.resize(start + length, 0);
            let mut walk = code;
            for position in (0..full).rev() {
                if position < length {
                    out[start + position] = self.suffix[walk];
                }
                walk = usize::from(self.prefix[walk]);
            }
            self.previous = Some(code);
        }
    }
}

struct Packer<'o> {
    out: &'o mut Vec<u8>,
    block_at: usize,
    buffer: u32,
    buffered: u32,
}

impl Packer<'_> {
    fn put(&mut self, code: usize, bits: u32) {
        self.buffer |= (code as u32) << self.buffered;
        self.buffered += bits;
        while self.buffered >= 8 {
            self.byte(self.buffer as u8);
            self.buffer >>= 8;
            self.buffered -= 8;
        }
    }

    fn byte(&mut self, byte: u8) {
        if self.out.len() - self.block_at > LONGEST_BLOCK {
            self.close_block();
        }
        self.out.push(byte);
    }

    fn close_block(&mut self) {
        self.out[self.block_at] = (self.out.len() - self.block_at - 1) as u8;
        self.block_at = self.out.len();
        self.out.push(0);
    }

    fn finish(mut self) {
        if self.buffered > 0 {
            self.byte(self.buffer as u8);
        }
        if self.out.len() - self.block_at > 1 {
            self.close_block();
        }
    }
}

#[derive(Debug)]
pub(crate) struct Encoder {
    keys: Vec<u32>,
    codes: Vec<u16>,
}

impl Encoder {
    pub(crate) fn new() -> Self {
        Self {
            keys: vec![EMPTY; SLOTS],
            codes: vec![0; SLOTS],
        }
    }

    pub(crate) fn encode(&mut self, indices: &[u8], min_bits: u8, out: &mut Vec<u8>) {
        let min_bits = u32::from(min_bits);
        let clear = 1usize << min_bits;
        let end = clear + 1;
        out.push(min_bits as u8);
        let block_at = out.len();
        out.push(0);
        let mut packer = Packer {
            out,
            block_at,
            buffer: 0,
            buffered: 0,
        };
        let mut code_bits = min_bits + 1;
        let mut next = clear + 2;
        self.keys.fill(EMPTY);
        packer.put(clear, code_bits);
        let mut indices = indices.iter();
        let Some(&first) = indices.next() else {
            packer.put(end, code_bits);
            packer.finish();
            return;
        };
        let mut current = usize::from(first);
        for &index in indices {
            let key = (current as u32) << 8 | u32::from(index);
            let mut slot = (key.wrapping_mul(HASH_MULTIPLIER) >> HASH_SHIFT) as usize;
            while self.keys[slot] != EMPTY && self.keys[slot] != key {
                slot = (slot + 1) & SLOT_MASK;
            }
            if self.keys[slot] == key {
                current = usize::from(self.codes[slot]);
                continue;
            }
            packer.put(current, code_bits);
            if next < TABLE {
                self.keys[slot] = key;
                self.codes[slot] = next as u16;
                next += 1;
                if next > 1 << code_bits && code_bits < MAX_BITS {
                    code_bits += 1;
                }
            } else {
                packer.put(clear, code_bits);
                self.keys.fill(EMPTY);
                code_bits = min_bits + 1;
                next = clear + 2;
            }
            current = usize::from(index);
        }
        packer.put(current, code_bits);
        if next == 1 << code_bits && code_bits < MAX_BITS {
            code_bits += 1;
        }
        packer.put(end, code_bits);
        packer.finish();
    }
}
