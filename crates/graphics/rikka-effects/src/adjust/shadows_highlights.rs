use crate::knob::{Scale, Unit, grouped};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

const AMOUNT: Scale = Scale::new(0.0..=100.0).unit(Unit::Percent);

const CORRECTION: Scale = Scale::new(-100.0..=100.0);

knobs! {
    for ShadowsHighlights;
    SHADOWS = "shadows" => shadows: || grouped(tr!("effects.adjust.shadows"), tr!("effects.adjust.amount")), AMOUNT;
    SHADOW_TONE = "shadow_tone" => shadow_tone: || {
            grouped(
                tr!("effects.adjust.shadows"),
                tr!("effects.shadows_highlights.tone"),
            )
        }, AMOUNT;
    HIGHLIGHTS = "highlights" => highlights: || {
            grouped(
                tr!("effects.adjust.highlights"),
                tr!("effects.adjust.amount"),
            )
        }, AMOUNT;
    HIGHLIGHT_TONE = "highlight_tone" => highlight_tone: || {
            grouped(
                tr!("effects.adjust.highlights"),
                tr!("effects.shadows_highlights.tone"),
            )
        }, AMOUNT;
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), Scale::new(0.0..=2500.0)
            .slide(0.0..=500.0)
            .unit(Unit::Pixels);
    COLOR = "color" => color: || tr!("effects.shadows_highlights.color"), CORRECTION;
    MIDTONES = "midtones" => midtones: || tr!("effects.shadows_highlights.midtone"), CORRECTION;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShadowsHighlights {
    pub shadows: f32,
    shadow_tone: f32,
    pub highlights: f32,
    highlight_tone: f32,
    radius: f32,
    color: f32,
    midtones: f32,
}

impl Default for ShadowsHighlights {
    fn default() -> ShadowsHighlights {
        ShadowsHighlights {
            shadows: 35.0,
            shadow_tone: 50.0,
            highlights: 0.0,
            highlight_tone: 50.0,
            radius: 30.0,
            color: 20.0,
            midtones: 0.0,
        }
    }
}

impl Parameters for ShadowsHighlights {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::ShadowsHighlights::new(self.shadows, self.highlights)
                .tone(self.shadow_tone, self.highlight_tone)
                .radius(self.radius)
                .color(self.color)
                .midtones(self.midtones)
                .into(),
        )
    }
}
