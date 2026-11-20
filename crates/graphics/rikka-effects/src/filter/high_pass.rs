use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for HighPass;
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), crate::filter::RADIUS;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HighPass {
    radius: f32,
}

impl Default for HighPass {
    fn default() -> HighPass {
        HighPass { radius: 10.0 }
    }
}

impl Parameters for HighPass {
    fn operation(&self) -> Option<Operation> {
        Some(imaging::HighPass::new(self.radius).into())
    }

    fn padding(&self) -> f32 {
        self.radius * 3.0
    }
}
