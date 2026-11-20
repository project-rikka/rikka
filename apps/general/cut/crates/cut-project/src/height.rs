use std::ops::RangeInclusive;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Height(u16);

impl Height {
    pub const SMALL: Height = Height(36);
    pub const MEDIUM: Height = Height(68);
    pub const LARGE: Height = Height(112);
    pub const PRESETS: [Height; 3] = [Height::SMALL, Height::MEDIUM, Height::LARGE];
    pub const RANGE: RangeInclusive<u16> = 28..=240;

    #[must_use]
    pub fn new(px: f32) -> Height {
        match px.is_finite() {
            true => Height(px.round().clamp(
                f32::from(*Height::RANGE.start()),
                f32::from(*Height::RANGE.end()),
            ) as u16),
            false => Height::default(),
        }
    }

    #[must_use]
    pub fn px(self) -> f32 {
        f32::from(self.0)
    }
}

impl Default for Height {
    fn default() -> Height {
        Height::MEDIUM
    }
}
