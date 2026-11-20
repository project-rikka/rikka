#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorType {
    L8,
    La8,
    Rgb8,
    Rgba8,
    L16,
    La16,
    Rgb16,
    Rgba16,
    Rgb32F,
    Rgba32F,
}

impl ColorType {
    #[must_use]
    pub const fn has_alpha(self) -> bool {
        matches!(
            self,
            ColorType::La8
                | ColorType::Rgba8
                | ColorType::La16
                | ColorType::Rgba16
                | ColorType::Rgba32F
        )
    }

    #[must_use]
    pub const fn has_color(self) -> bool {
        !matches!(
            self,
            ColorType::L8 | ColorType::La8 | ColorType::L16 | ColorType::La16
        )
    }

    #[must_use]
    pub const fn channel_count(self) -> u8 {
        match self {
            ColorType::L8 | ColorType::L16 => 1,
            ColorType::La8 | ColorType::La16 => 2,
            ColorType::Rgb8 | ColorType::Rgb16 | ColorType::Rgb32F => 3,
            ColorType::Rgba8 | ColorType::Rgba16 | ColorType::Rgba32F => 4,
        }
    }

    #[must_use]
    pub const fn bits_per_channel(self) -> u8 {
        match self {
            ColorType::L8 | ColorType::La8 | ColorType::Rgb8 | ColorType::Rgba8 => 8,
            ColorType::L16 | ColorType::La16 | ColorType::Rgb16 | ColorType::Rgba16 => 16,
            ColorType::Rgb32F | ColorType::Rgba32F => 32,
        }
    }

    #[must_use]
    pub const fn bytes_per_pixel(self) -> u8 {
        self.channel_count() * self.bits_per_channel() / 8
    }

    pub(crate) const fn with_alpha(self) -> ColorType {
        match self {
            ColorType::L8 => ColorType::La8,
            ColorType::Rgb8 => ColorType::Rgba8,
            ColorType::L16 => ColorType::La16,
            ColorType::Rgb16 => ColorType::Rgba16,
            ColorType::Rgb32F => ColorType::Rgba32F,
            already_has_one => already_has_one,
        }
    }

    pub(crate) const fn integer(gray: bool, alpha: bool, deep: bool) -> ColorType {
        match (gray, alpha, deep) {
            (true, false, false) => ColorType::L8,
            (true, true, false) => ColorType::La8,
            (false, false, false) => ColorType::Rgb8,
            (false, true, false) => ColorType::Rgba8,
            (true, false, true) => ColorType::L16,
            (true, true, true) => ColorType::La16,
            (false, false, true) => ColorType::Rgb16,
            (false, true, true) => ColorType::Rgba16,
        }
    }
}
