#[must_use]
pub fn strip_emulation_prevention(escaped: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(escaped.len());
    let mut zeros = 0;
    for &byte in escaped {
        if zeros >= 2 && byte == 3 {
            zeros = 0;
            continue;
        }
        zeros = if byte == 0 { zeros + 1 } else { 0 };
        raw.push(byte);
    }
    raw
}

#[must_use]
pub fn insert_emulation_prevention(raw: &[u8]) -> Vec<u8> {
    let mut escaped = Vec::with_capacity(raw.len() + raw.len() / 64);
    let mut zeros = 0;
    for &byte in raw {
        if zeros >= 2 && byte <= 3 {
            escaped.push(3);
            zeros = 0;
        }
        zeros = if byte == 0 { zeros + 1 } else { 0 };
        escaped.push(byte);
    }
    escaped
}

#[derive(Clone, Debug)]
pub struct BitReader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> BitReader<'a> {
    #[must_use]
    pub fn new(bytes: &'a [u8]) -> BitReader<'a> {
        BitReader { bytes, at: 0 }
    }

    pub fn bit(&mut self) -> Option<u32> {
        let byte = *self.bytes.get(self.at / 8)?;
        let bit = (byte >> (7 - self.at % 8)) & 1;
        self.at += 1;
        Some(u32::from(bit))
    }

    pub fn bits(&mut self, count: u32) -> Option<u32> {
        let mut value = 0u32;
        for _ in 0..count {
            value = value << 1 | self.bit()?;
        }
        Some(value)
    }

    pub fn golomb(&mut self) -> Option<u32> {
        let mut zeros = 0;
        while self.bit()? == 0 {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        let rest = self.bits(zeros)?;
        Some(((1u64 << zeros) - 1 + u64::from(rest)) as u32)
    }

    pub fn signed_golomb(&mut self) -> Option<i32> {
        let code = self.golomb()?;
        let magnitude = code.div_ceil(2) as i32;
        Some(if code % 2 == 1 { magnitude } else { -magnitude })
    }
}

#[derive(Clone, Debug, Default)]
pub struct BitWriter {
    bytes: Vec<u8>,
    filled: u32,
}

impl BitWriter {
    pub fn bit(&mut self, bit: u32) {
        if self.filled == 0 {
            self.bytes.push(0);
        }
        if bit & 1 == 1
            && let Some(last) = self.bytes.last_mut()
        {
            *last |= 1 << (7 - self.filled);
        }
        self.filled = (self.filled + 1) % 8;
    }

    pub fn bits(&mut self, value: u32, count: u32) {
        for shift in (0..count).rev() {
            self.bit(value >> shift);
        }
    }

    pub fn golomb(&mut self, value: u32) {
        let coded = u64::from(value) + 1;
        let width = 64 - coded.leading_zeros();
        for _ in 1..width {
            self.bit(0);
        }
        for shift in (0..width).rev() {
            self.bit((coded >> shift) as u32);
        }
    }

    pub fn signed_golomb(&mut self, value: i32) {
        let code = if value > 0 {
            value as u32 * 2 - 1
        } else {
            value.unsigned_abs() * 2
        };
        self.golomb(code);
    }

    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        self.bit(1);
        while self.filled != 0 {
            self.bit(0);
        }
        self.bytes
    }
}
