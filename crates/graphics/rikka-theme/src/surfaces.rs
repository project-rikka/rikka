use rikka_types::Color;

use crate::tone;
use crate::variant::Variant;

pub(crate) struct ToneSteps {
    pub(crate) surface: f32,
    pub(crate) variant: f32,
    pub(crate) selection: f32,
}

const LIGHT: ToneSteps = ToneSteps {
    surface: 2.0,
    variant: 4.0,
    selection: 8.0,
};

const DARK: ToneSteps = ToneSteps {
    surface: 3.0,
    variant: 7.0,
    selection: 18.0,
};

const SELECTION_CHROMA: f64 = 20.0;

pub(crate) struct Surfaces {
    pub(crate) background: Color,
    pub(crate) surface: Color,
    pub(crate) surface_variant: Color,
    variant: Variant,
}

impl Surfaces {
    pub(crate) fn derive(
        background: Color,
        surface: Color,
        surface_variant: Color,
        variant: Variant,
    ) -> Surfaces {
        let tone_steps = tone_steps(variant);
        Surfaces {
            background,
            surface: layer(surface, background, tone_steps.surface),
            surface_variant: layer(surface_variant, background, tone_steps.variant),
            variant,
        }
    }

    pub(crate) fn selection(&self, accent: Color) -> Color {
        selection(self.background, self.variant, accent)
    }
}

pub(crate) fn selection(background: Color, variant: Variant, accent: Color) -> Color {
    let ground = tone::measure(background);
    let step = tone_steps(variant).selection;
    let step = if variant.is_dark() { step } else { -step };
    tone::muted(accent, offset_tone(ground, step), SELECTION_CHROMA)
}

pub(crate) const fn tone_steps(variant: Variant) -> &'static ToneSteps {
    match variant {
        Variant::Dark => &DARK,
        Variant::Light => &LIGHT,
    }
}

fn layer(color: Color, ground: Color, step: f32) -> Color {
    let ground = tone::measure(ground);
    let rises = tone::measure(color) > ground;
    tone::with_tone(color, offset_tone(ground, if rises { step } else { -step }))
}

fn offset_tone(ground: f32, step: f32) -> f32 {
    let target = ground + step;
    if (0.0..=100.0).contains(&target) {
        target
    } else {
        ground - step
    }
}
