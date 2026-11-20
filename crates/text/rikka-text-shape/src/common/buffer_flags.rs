use std::ops::{BitOr, BitOrAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BufferFlags(u32);

impl BufferFlags {
    pub const BEGINNING_OF_TEXT: Self = Self(0x01);
    pub const END_OF_TEXT: Self = Self(0x02);
    pub const PRESERVE_DEFAULT_IGNORABLES: Self = Self(0x04);
    pub const REMOVE_DEFAULT_IGNORABLES: Self = Self(0x08);
    pub const DO_NOT_INSERT_DOTTED_CIRCLE: Self = Self(0x10);
    pub const PRODUCE_UNSAFE_TO_CONCAT: Self = Self(0x20);
    pub const PRODUCE_SAFE_TO_INSERT_TATWEEL: Self = Self(0x40);

    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for BufferFlags {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOrAssign for BufferFlags {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}
