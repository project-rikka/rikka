use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

const NEUTRAL: f32 = 6504.0;

knobs! {
    for WhiteBalance;
    TEMPERATURE = "temperature" => temperature: || tr!("effects.adjust.temperature"), Scale::new(1667.0..=25000.0)
            .slide(2000.0..=12000.0)
            .unit(Unit::Kelvin);
    TINT = "tint" => tint: || tr!("effects.white_balance.tint"), Scale::new(-150.0..=150.0);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WhiteBalance {
    temperature: f32,
    tint: f32,
}

impl Default for WhiteBalance {
    fn default() -> WhiteBalance {
        WhiteBalance {
            temperature: NEUTRAL,
            tint: 0.0,
        }
    }
}

impl Parameters for WhiteBalance {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::WhiteBalance::new(self.temperature)
                .tint(self.tint)
                .into(),
        )
    }
}
