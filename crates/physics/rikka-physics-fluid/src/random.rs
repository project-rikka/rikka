#[derive(Clone, Copy, Debug)]
pub(crate) struct Random(u64);

impl Random {
    pub(crate) fn new(parts: &[u64]) -> Random {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        for part in parts {
            state = mix(state ^ part.wrapping_mul(0xbf58_476d_1ce4_e5b9));
        }
        Random(state)
    }

    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.0)
    }

    pub(crate) fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 * (1.0 / 16_777_216.0)
    }

    pub(crate) fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
