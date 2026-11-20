pub(crate) struct Random(u64);

impl Random {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }

    pub(crate) fn below(&mut self, bound: usize) -> usize {
        self.next() as usize % bound
    }
}

pub(crate) fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut random = Random::new(seed);
    (0..len).map(|_| random.next() as u8).collect()
}

pub(crate) fn prose(len: usize, seed: u64) -> Vec<u8> {
    const WORDS: [&str; 12] = [
        "window",
        "surface",
        "buffer",
        "frame",
        "the",
        "of",
        "compositor",
        "pixel",
        "and",
        "damage",
        "commit",
        "a",
    ];
    let mut random = Random::new(seed);
    let mut text = Vec::with_capacity(len + 16);
    while text.len() < len {
        text.extend_from_slice(WORDS[random.below(WORDS.len())].as_bytes());
        text.push(if random.below(9) == 0 { b'\n' } else { b' ' });
    }
    text.truncate(len);
    text
}

pub(crate) fn far_repeats(len: usize, seed: u64) -> Vec<u8> {
    let mut data = noise(32768, seed);
    let mut random = Random::new(seed ^ 0x5DEECE66D);
    while data.len() < len {
        let length = 3 + random.below(300);
        let distance = 1 + random.below(32768.min(data.len()));
        let start = data.len() - distance;
        for offset in 0..length {
            data.push(data[start + offset]);
        }
        data.push(random.next() as u8);
    }
    data.truncate(len);
    data
}

pub(crate) fn samples() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("empty", Vec::new()),
        ("one byte", vec![7]),
        ("two bytes", vec![7, 7]),
        ("run", vec![0x55; 100_000]),
        ("short period", b"abc".repeat(40_000)),
        ("noise", noise(70_000, 1)),
        ("prose", prose(300_000, 2)),
        ("far repeats", far_repeats(200_000, 3)),
        ("ramp", (0..=255u8).cycle().take(66_000).collect()),
    ]
}
