use crate::parse::array::Scalar;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct F2Dot14(pub i16);

impl F2Dot14 {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(0x4000);
    pub const MINUS_ONE: Self = Self(-0x4000);

    #[inline]
    pub fn from_f32(value: f32) -> Self {
        Self((value * 16384.0).round().clamp(-32768.0, 32767.0) as i16)
    }

    #[inline]
    pub fn to_f32(self) -> f32 {
        f32::from(self.0) / 16384.0
    }
}

impl Scalar for F2Dot14 {
    const SIZE: usize = 2;

    #[inline]
    fn parse(bytes: &[u8]) -> Option<Self> {
        bytes.first_chunk().map(|b| Self(i16::from_be_bytes(*b)))
    }
}
