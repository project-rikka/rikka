use rikka_physics_math::Vec3;

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug)]
pub struct Fnv1a(u64);

impl Default for Fnv1a {
    fn default() -> Self {
        Self::new()
    }
}

impl Fnv1a {
    #[inline]
    pub const fn new() -> Self {
        Self(OFFSET)
    }

    #[inline]
    pub fn u32(&mut self, value: u32) {
        self.0 = (self.0 ^ u64::from(value)).wrapping_mul(PRIME);
    }

    #[inline]
    pub fn u64(&mut self, value: u64) {
        self.u32(value as u32);
        self.u32((value >> 32) as u32);
    }

    #[inline]
    pub fn f32(&mut self, value: f32) {
        self.u32(value.to_bits());
    }

    #[inline]
    pub fn vec3(&mut self, value: Vec3) {
        for component in value.to_array() {
            self.f32(component);
        }
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        let (words, remainder) = bytes.as_chunks::<4>();
        for &word in words {
            self.u32(u32::from_le_bytes(word));
        }
        for &byte in remainder {
            self.u32(u32::from(byte));
        }
        self.u64(bytes.len() as u64);
    }

    #[inline]
    pub fn finish(self) -> u64 {
        self.0
    }
}
