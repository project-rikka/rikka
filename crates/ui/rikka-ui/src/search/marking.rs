use rikka_theme::Palette;
use rikka_types::Color;

const REST: f32 = 0.28;
const CURRENT: f32 = 0.62;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marking {
    pub rest: Color,
    pub current: Color,
}

impl Marking {
    #[must_use]
    pub fn from_palette(palette: Palette) -> Self {
        Self {
            rest: palette.warning.scale_alpha(REST),
            current: palette.warning.scale_alpha(CURRENT),
        }
    }

    #[must_use]
    pub fn color(self, current: bool) -> Color {
        match current {
            true => self.current,
            false => self.rest,
        }
    }
}
