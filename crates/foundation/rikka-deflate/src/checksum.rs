use rikka_simd::prelude::*;

const ADLER_MODULUS: u32 = 65521;
const ADLER_CHUNK: usize = 5536;
const CRC_POLYNOMIAL: u32 = 0xEDB8_8320;
const SLICES: usize = 16;

static CRC_TABLES: [[u32; 256]; SLICES] = {
    let mut tables = [[0u32; 256]; SLICES];
    let mut index = 0;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 == 1 {
                (value >> 1) ^ CRC_POLYNOMIAL
            } else {
                value >> 1
            };
            bit += 1;
        }
        tables[0][index] = value;
        index += 1;
    }
    let mut slice = 1;
    while slice < SLICES {
        let mut index = 0;
        while index < 256 {
            let previous = tables[slice - 1][index];
            tables[slice][index] = (previous >> 8) ^ tables[0][(previous & 0xFF) as usize];
            index += 1;
        }
        slice += 1;
    }
    tables
};

#[must_use]
pub fn adler32(data: &[u8]) -> u32 {
    let mut adler = Adler32::new();
    adler.update(data);
    adler.finish()
}

#[derive(Clone, Copy, Debug)]
pub struct Adler32 {
    a: u64,
    b: u64,
}

impl Adler32 {
    #[must_use]
    pub const fn new() -> Adler32 {
        Adler32 { a: 1, b: 0 }
    }

    pub fn update(&mut self, data: &[u8]) {
        let (a, b) = (self.a, self.b);
        (self.a, self.b) =
            rikka_simd::dispatch!(Level::detect(), simd => adler32_lanes(simd, a, b, data));
    }

    #[must_use]
    pub const fn finish(self) -> u32 {
        ((self.b as u32) << 16) | self.a as u32
    }
}

impl Default for Adler32 {
    fn default() -> Adler32 {
        Adler32::new()
    }
}

#[inline(always)]
fn adler32_lanes<S: Simd>(_: S, mut a: u64, mut b: u64, data: &[u8]) -> (u64, u64) {
    let lanes = S::U8s::LANES;
    let block = ADLER_CHUNK / lanes * lanes;
    let mut weights = [0u16; 64];
    for (index, weight) in weights.iter_mut().enumerate() {
        *weight = (lanes - index.min(lanes)) as u16;
    }
    let low_weights = S::U16s::load(&weights[..lanes / 2]);
    let high_weights = S::U16s::load(&weights[lanes / 2..lanes]);
    let (whole, tail) = data.split_at(data.len() / lanes * lanes);
    for chunk in whole.chunks(block) {
        let mut sums = S::U32s::splat(0);
        let mut before = S::U32s::splat(0);
        let mut weighted = S::U32s::splat(0);
        for bytes in chunk.chunks_exact(lanes) {
            let bytes = S::U8s::load(bytes);
            let (low, high) = (bytes.widen_low(), bytes.widen_high());
            before += sums;
            let total = low + high;
            sums += total.widen_low() + total.widen_high();
            let products = low * low_weights + high * high_weights;
            weighted += products.widen_low() + products.widen_high();
        }
        let count = (chunk.len() / lanes) as u64;
        let total = |lanes: S::U32s| u64::from(lanes.reduce_add());
        b += count * lanes as u64 * a + total(before) * lanes as u64 + total(weighted);
        a += total(sums);
        a %= u64::from(ADLER_MODULUS);
        b %= u64::from(ADLER_MODULUS);
    }
    for &byte in tail {
        a += u64::from(byte);
        b += a;
    }
    (a % u64::from(ADLER_MODULUS), b % u64::from(ADLER_MODULUS))
}

#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = Crc32::new();
    crc.update(data);
    crc.finish()
}

#[derive(Clone, Copy, Debug)]
pub struct Crc32 {
    state: u32,
}

impl Crc32 {
    #[must_use]
    pub const fn new() -> Crc32 {
        Crc32 { state: !0 }
    }

    pub fn update(&mut self, data: &[u8]) {
        let mut state = self.state;
        let (words, tail) = data.as_chunks::<SLICES>();
        for word in words {
            let first = u32::from_le_bytes([word[0], word[1], word[2], word[3]]) ^ state;
            let mut folded = 0;
            for (at, byte) in first
                .to_le_bytes()
                .into_iter()
                .chain(word[4..].iter().copied())
                .enumerate()
            {
                folded ^= CRC_TABLES[SLICES - 1 - at][usize::from(byte)];
            }
            state = folded;
        }
        for &byte in tail {
            state = CRC_TABLES[0][usize::from(state as u8 ^ byte)] ^ (state >> 8);
        }
        self.state = state;
    }

    #[must_use]
    pub const fn finish(self) -> u32 {
        !self.state
    }
}

impl Default for Crc32 {
    fn default() -> Crc32 {
        Crc32::new()
    }
}
