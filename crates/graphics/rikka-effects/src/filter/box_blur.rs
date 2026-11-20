use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for BoxBlur;
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), Scale::new(1.0..=2000.0)
        .slide(1.0..=250.0)
        .unit(Unit::Pixels);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoxBlur {
    radius: f32,
}

impl Default for BoxBlur {
    fn default() -> BoxBlur {
        BoxBlur { radius: 10.0 }
    }
}

impl Parameters for BoxBlur {
    fn operation(&self) -> Option<Operation> {
        Some(imaging::BoxBlur::new(self.radius).into())
    }

    fn padding(&self) -> f32 {
        self.radius
    }
}
