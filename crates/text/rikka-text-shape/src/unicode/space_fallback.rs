#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum SpaceKind {
    NotSpace = 0,
    Em = 1,
    Em2 = 2,
    Em3 = 3,
    Em4 = 4,
    Em5 = 5,
    Em6 = 6,
    Em16 = 16,
    FourEm18 = 17,
    Space = 18,
    Figure = 19,
    Punctuation = 20,
    Narrow = 21,
}

impl SpaceKind {
    pub(crate) fn from_codepoint(codepoint: u32) -> Self {
        match codepoint {
            0x0020 | 0x00A0 => Self::Space,
            0x2000 | 0x2002 => Self::Em2,
            0x2001 | 0x2003 | 0x3000 => Self::Em,
            0x2004 => Self::Em3,
            0x2005 => Self::Em4,
            0x2006 => Self::Em6,
            0x2007 => Self::Figure,
            0x2008 => Self::Punctuation,
            0x2009 => Self::Em5,
            0x200A => Self::Em16,
            0x202F => Self::Narrow,
            0x205F => Self::FourEm18,
            _ => Self::NotSpace,
        }
    }

    pub(crate) fn from_bits(bits: u8) -> Self {
        match bits {
            1 => Self::Em,
            2 => Self::Em2,
            3 => Self::Em3,
            4 => Self::Em4,
            5 => Self::Em5,
            6 => Self::Em6,
            16 => Self::Em16,
            17 => Self::FourEm18,
            18 => Self::Space,
            19 => Self::Figure,
            20 => Self::Punctuation,
            21 => Self::Narrow,
            _ => Self::NotSpace,
        }
    }
}
