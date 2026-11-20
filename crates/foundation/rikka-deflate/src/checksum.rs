const ADLER_MODULUS: u32 = 65521;
const ADLER_BLOCK: usize = 32;
const ADLER_CHUNK: usize = 5536;
const CRC_POLYNOMIAL: u32 = 0xEDB8_8320;
const SLICES: usize = 8;

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
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(ADLER_CHUNK) {
        let (blocks, tail) = chunk.as_chunks::<ADLER_BLOCK>();
        for block in blocks {
            let mut sum = 0u32;
            let mut weighted = 0u32;
            for (position, &byte) in block.iter().enumerate() {
                sum += u32::from(byte);
                weighted += (ADLER_BLOCK - position) as u32 * u32::from(byte);
            }
            b += ADLER_BLOCK as u32 * a + weighted;
            a += sum;
        }
        for &byte in tail {
            a += u32::from(byte);
            b += a;
        }
        a %= ADLER_MODULUS;
        b %= ADLER_MODULUS;
    }
    (b << 16) | a
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
            let low = u32::from_le_bytes([word[0], word[1], word[2], word[3]]) ^ state;
            let high = u32::from_le_bytes([word[4], word[5], word[6], word[7]]);
            state = CRC_TABLES[7][(low & 0xFF) as usize]
                ^ CRC_TABLES[6][((low >> 8) & 0xFF) as usize]
                ^ CRC_TABLES[5][((low >> 16) & 0xFF) as usize]
                ^ CRC_TABLES[4][(low >> 24) as usize]
                ^ CRC_TABLES[3][(high & 0xFF) as usize]
                ^ CRC_TABLES[2][((high >> 8) & 0xFF) as usize]
                ^ CRC_TABLES[1][((high >> 16) & 0xFF) as usize]
                ^ CRC_TABLES[0][(high >> 24) as usize];
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
