use rikka_types::Color;

use crate::spec::ThemeSpec;
use crate::tone;
use crate::variant::Variant;

const RIM_CHROMA: f64 = 24.0;

const LIQUID_RIM_SHARE: f32 = 0.7;

struct Pane {
    glass_opacity: f32,
    liquid_opacity: f32,
    rim_tone: f32,
    rim_alpha: f32,
}

const DARK: Pane = Pane {
    glass_opacity: 0.42,
    liquid_opacity: 0.30,
    rim_tone: 90.0,
    rim_alpha: 0.20,
};

const LIGHT: Pane = Pane {
    glass_opacity: 0.48,
    liquid_opacity: 0.36,
    rim_tone: 30.0,
    rim_alpha: 0.16,
};

const fn pane(variant: Variant) -> &'static Pane {
    match variant {
        Variant::Dark => &DARK,
        Variant::Light => &LIGHT,
    }
}

impl ThemeSpec {
    #[must_use]
    pub fn glass(mut self) -> ThemeSpec {
        let pane = pane(self.variant);
        self.opacity = self.opacity.min(pane.glass_opacity);
        self.colors.border = rim(self.colors.accent, pane);
        self
    }

    #[must_use]
    pub fn liquid(self) -> ThemeSpec {
        let mut spec = self.glass();
        spec.opacity = spec.opacity.min(pane(spec.variant).liquid_opacity);
        spec.lens.get_or_insert_default();
        spec.colors.border = spec.colors.border.scale_alpha(LIQUID_RIM_SHARE);
        spec
    }
}

fn rim(accent: Color, pane: &Pane) -> Color {
    tone::muted(accent, pane.rim_tone, RIM_CHROMA).with_alpha(pane.rim_alpha)
}
