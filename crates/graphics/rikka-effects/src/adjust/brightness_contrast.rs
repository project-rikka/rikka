use crate::knob::{Knob, Scale};
use crate::parameters::{KnobKey, Knobs, Parameters};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

const BRIGHTNESS: Scale = Scale::new(-150.0..=150.0);

const CONTRAST: Scale = Scale::new(-50.0..=100.0);

const LEGACY_CONTRAST: Scale = Scale::new(-100.0..=100.0);

pub const BRIGHTNESS_KNOB: KnobKey<BrightnessContrast> = KnobKey::new("brightness");

pub const CONTRAST_KNOB: KnobKey<BrightnessContrast> = KnobKey::new("contrast");

const KNOBS: &[Knob] = &knobs_with(CONTRAST);

const LEGACY_KNOBS: &[Knob] = &knobs_with(LEGACY_CONTRAST);

pub const fn knobs_with(contrast: Scale) -> [Knob; 2] {
    [
        Knob::new(
            BRIGHTNESS_KNOB.name(),
            || tr!("effects.adjust.brightness"),
            BRIGHTNESS,
        ),
        Knob::new(
            CONTRAST_KNOB.name(),
            || tr!("effects.adjust.contrast"),
            contrast,
        ),
    ]
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrightnessContrast {
    brightness: f32,
    contrast: f32,
    pub legacy: bool,
}

impl Knobs for BrightnessContrast {
    fn knobs(&self) -> &'static [Knob] {
        match self.legacy {
            true => LEGACY_KNOBS,
            false => KNOBS,
        }
    }

    fn slot(&mut self, key: &str) -> Option<&mut f32> {
        match key {
            _ if key == BRIGHTNESS_KNOB.name() => Some(&mut self.brightness),
            _ if key == CONTRAST_KNOB.name() => Some(&mut self.contrast),
            _ => None,
        }
    }

    fn peek(&self, key: &str) -> Option<f32> {
        self.clone().slot(key).copied()
    }
}

impl Parameters for BrightnessContrast {
    fn normalize(&mut self, _changed: Option<&str>) {
        let contrast = match self.legacy {
            true => &LEGACY_CONTRAST,
            false => &CONTRAST,
        };
        self.contrast = contrast.clamp(self.contrast);
    }

    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::BrightnessContrast::new(self.brightness, self.contrast)
                .legacy(self.legacy)
                .into(),
        )
    }
}
