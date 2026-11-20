use rikka_types::{Color, Lens};

use crate::palette::Palette;

const GROUND: f32 = 1.0;
const SURFACE: f32 = 0.88;
const VARIANT: f32 = 0.80;
const SELECTION: f32 = 0.55;
const OUTLINE: f32 = 0.50;

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct Translucency {
    opacity: f32,
    radius: f32,
    lens: Option<Lens>,
}

impl Translucency {
    pub const MAX_BLUR: f32 = 128.0;

    pub const SCRIM_BLUR: f32 = 32.0;

    pub const fn opaque() -> Translucency {
        Translucency {
            opacity: 1.0,
            radius: 0.0,
            lens: None,
        }
    }

    pub fn new(opacity: f32) -> Translucency {
        Translucency {
            opacity: opacity.clamp(0.0, 1.0),
            radius: 0.0,
            lens: None,
        }
    }

    pub fn with_blur(mut self, radius: f32) -> Translucency {
        self.radius = radius.clamp(0.0, Translucency::MAX_BLUR);
        self
    }

    pub fn with_lens(mut self, lens: Option<Lens>) -> Translucency {
        self.lens = lens;
        self
    }

    pub fn combine(self, session: Translucency) -> Translucency {
        Translucency {
            opacity: self.opacity * session.opacity,
            radius: session.radius,
            lens: self.lens,
        }
    }

    #[must_use]
    pub fn opacity(self) -> f32 {
        self.opacity
    }

    #[must_use]
    pub fn is_translucent(self) -> bool {
        self.opacity < 1.0
    }

    #[must_use]
    pub fn blur_radius(self) -> Option<f32> {
        (self.is_translucent() && self.radius > 0.0).then_some(self.radius)
    }

    #[must_use]
    pub fn lens(self) -> Option<Lens> {
        self.lens.filter(|_| self.blur_radius().is_some())
    }

    #[must_use]
    pub fn declared_lens(self) -> Option<Lens> {
        self.lens
    }

    pub(crate) fn derive(self, palette: Palette) -> Palette {
        if !self.is_translucent() {
            return palette;
        }
        Palette {
            background: self.fade(palette.background, GROUND),
            surface: self.fade(palette.surface, SURFACE),
            surface_variant: self.fade(palette.surface_variant, VARIANT),
            selection: self.fade(palette.selection, SELECTION),
            outline: self.fade(palette.outline, OUTLINE),
            ..palette
        }
    }

    fn fade(self, color: Color, share: f32) -> Color {
        color.scale_alpha(1.0 - (1.0 - self.opacity) * share)
    }
}
