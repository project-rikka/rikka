#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size {
    Normal,
    Large,
    XLarge,
    XxLarge,
}

impl Size {
    #[must_use]
    pub const fn pixels(self) -> u32 {
        match self {
            Self::Normal => 128,
            Self::Large => 256,
            Self::XLarge => 512,
            Self::XxLarge => 1024,
        }
    }

    pub(crate) const fn directory(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Large => "large",
            Self::XLarge => "x-large",
            Self::XxLarge => "xx-large",
        }
    }

    #[must_use]
    pub fn covering(min_pixels: u32) -> Self {
        [Self::Normal, Self::Large, Self::XLarge, Self::XxLarge]
            .into_iter()
            .find(|size| size.pixels() >= min_pixels)
            .unwrap_or(Self::XxLarge)
    }
}
