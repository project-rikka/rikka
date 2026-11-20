use crate::parse::array::Scalar;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlyphId(pub u16);

impl GlyphId {
    pub const NOTDEF: Self = Self(0);

    #[inline]
    pub const fn to_u16(self) -> u16 {
        self.0
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl From<u16> for GlyphId {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl Scalar for GlyphId {
    const SIZE: usize = 2;

    #[inline]
    fn parse(bytes: &[u8]) -> Option<Self> {
        bytes.first_chunk().map(|b| Self(u16::from_be_bytes(*b)))
    }
}
