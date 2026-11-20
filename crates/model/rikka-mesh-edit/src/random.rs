use crate::math::NOISE_HASH;

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    #[must_use]
    pub fn new(seed: u32) -> Rng {
        Rng {
            state: (u64::from(seed) << 16) | 0x330E,
        }
    }

    #[must_use]
    pub fn new_random(seed: u32) -> Rng {
        let mut rng = Rng::new(seed.wrapping_add(u32::from(NOISE_HASH[(seed & 255) as usize])));
        let next = rng.next_u32();
        rng = Rng::new(next.wrapping_add(u32::from(NOISE_HASH[(next & 255) as usize])));
        let next = rng.next_u32();
        Rng::new(next.wrapping_add(u32::from(NOISE_HASH[(next & 255) as usize])))
    }

    fn step(&mut self) {
        self.state =
            0x5_DEEC_E66D_u64.wrapping_mul(self.state).wrapping_add(0xB) & 0xFFFF_FFFF_FFFF;
    }

    pub fn next_u32(&mut self) -> u32 {
        self.step();
        (self.state >> 17) as u32
    }

    pub fn next_f32(&mut self) -> f32 {
        self.step();
        ((self.state >> 17) as u32 as i32) as f32 / 2_147_483_648.0
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        let mut index = items.len();
        while index > 1 {
            index -= 1;
            let other = (self.next_u32() as usize) % (index + 1);
            items.swap(index, other);
        }
    }
}
