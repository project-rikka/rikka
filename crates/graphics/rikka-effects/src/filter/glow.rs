use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for Glow;
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), Scale::new(0.0..=500.0)
            .slide(0.0..=200.0)
            .unit(Unit::Pixels);
    BRIGHTNESS = "brightness" => brightness: || tr!("effects.adjust.brightness"), Scale::new(0.0..=400.0).unit(Unit::Percent);
    THRESHOLD = "threshold" => threshold: || tr!("effects.adjust.threshold"), crate::filter::PERCENT;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Glow {
    radius: f32,
    brightness: f32,
    threshold: f32,
}

impl Default for Glow {
    fn default() -> Glow {
        Glow {
            radius: 10.0,
            brightness: 100.0,
            threshold: 60.0,
        }
    }
}

impl Parameters for Glow {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::Glow::new(self.radius)
                .brightness(self.brightness / 100.0)
                .threshold(self.threshold / 100.0)
                .into(),
        )
    }

    fn padding(&self) -> f32 {
        self.radius * 3.0
    }
}
