use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for GaussianBlur;
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), crate::filter::RADIUS;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GaussianBlur {
    radius: f32,
}

impl Default for GaussianBlur {
    fn default() -> GaussianBlur {
        GaussianBlur { radius: 1.0 }
    }
}

impl Parameters for GaussianBlur {
    fn operation(&self) -> Option<Operation> {
        Some(imaging::GaussianBlur::new(self.radius).into())
    }

    fn padding(&self) -> f32 {
        self.radius * 3.0
    }
}
