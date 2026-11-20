use crate::knob::Scale;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for ReduceNoise;
    STRENGTH = "strength" => strength: || tr!("effects.reduce_noise.strength"), Scale::new(0.0..=10.0);
    PRESERVE_DETAILS = "preserve_details" => preserve_details: || tr!("effects.reduce_noise.preserve_details"), crate::filter::PERCENT;
    COLOR = "color" => color: || tr!("effects.reduce_noise.color"), crate::filter::PERCENT;
    SHARPEN_DETAILS = "sharpen_details" => sharpen_details: || tr!("effects.reduce_noise.sharpen_details"), crate::filter::PERCENT;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReduceNoise {
    strength: f32,
    preserve_details: f32,
    color: f32,
    sharpen_details: f32,
}

impl Default for ReduceNoise {
    fn default() -> ReduceNoise {
        ReduceNoise {
            strength: 6.0,
            preserve_details: 60.0,
            color: 45.0,
            sharpen_details: 25.0,
        }
    }
}

impl Parameters for ReduceNoise {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::ReduceNoise::new(self.strength)
                .preserve_details(self.preserve_details)
                .color(self.color)
                .sharpen_details(self.sharpen_details)
                .into(),
        )
    }
}
