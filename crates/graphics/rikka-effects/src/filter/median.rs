use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for Median;
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), Scale::new(1.0..=500.0)
        .slide(1.0..=100.0)
        .unit(Unit::Pixels);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Median {
    radius: f32,
}

impl Default for Median {
    fn default() -> Median {
        Median { radius: 1.0 }
    }
}

impl Parameters for Median {
    fn operation(&self) -> Option<Operation> {
        Some(imaging::Median::new(self.radius).into())
    }

    fn padding(&self) -> f32 {
        self.radius
    }
}
