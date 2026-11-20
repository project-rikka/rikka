#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorIndex {
    Palette(u16),
    Foreground,
}

impl ColorIndex {
    const FOREGROUND: u16 = 0xFFFF;

    #[inline]
    pub(crate) fn from_raw(index: u16) -> Self {
        if index == Self::FOREGROUND {
            Self::Foreground
        } else {
            Self::Palette(index)
        }
    }
}
