use rikka_theme::Palette;
use rikka_types::Color;

const HOVER_ALPHA: f32 = 0.08;
const VEIL_ALPHA: f32 = 0.12;
const WASH_ALPHA: f32 = 0.34;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Colours {
    pub(crate) panel: Color,
    pub(crate) card: Color,
    pub(crate) zone: Color,
    pub(crate) chosen: Color,
    pub(crate) on_chosen: Color,
    pub(crate) hover: Color,
    pub(crate) outline: Color,
    pub(crate) text: Color,
}

impl Colours {
    pub(crate) fn from_palette(palette: Palette) -> Self {
        Self {
            panel: palette.background,
            card: palette.surface,
            zone: palette.surface_variant,
            chosen: palette.selection,
            on_chosen: palette.on_selection,
            hover: palette.overlay.with_alpha(HOVER_ALPHA),
            outline: palette.outline,
            text: palette.text,
        }
    }

    pub(crate) fn veil(self, blurred: bool) -> Color {
        let alpha = if blurred { VEIL_ALPHA } else { WASH_ALPHA };
        self.card.with_alpha(alpha)
    }

    pub(crate) fn ink(self, chosen: bool) -> Color {
        if chosen { self.on_chosen } else { self.text }
    }
}
