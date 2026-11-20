use glam::Vec3;

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    #[must_use]
    pub fn new(seed: u64) -> Rng {
        Rng {
            state: seed ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    pub fn below(&mut self, bound: usize) -> usize {
        match bound {
            0 => 0,
            _ => (self.next_u64() % bound as u64) as usize,
        }
    }

    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }

    pub fn int(&mut self, low: u32, high: u32) -> u32 {
        low + self.below((high - low + 1) as usize) as u32
    }

    pub fn chance(&mut self, probability: f32) -> bool {
        self.unit() < probability
    }

    pub fn flip(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }

    pub fn direction(&mut self) -> Vec3 {
        let value = Vec3::new(
            self.range(-1.0, 1.0),
            self.range(-1.0, 1.0),
            self.range(-1.0, 1.0),
        );
        value.try_normalize().unwrap_or(Vec3::Z)
    }
}
