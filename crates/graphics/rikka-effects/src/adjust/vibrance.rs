use crate::knob::Scale;
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

const AMOUNT: Scale = Scale::new(-100.0..=100.0);

knobs! {
    for Vibrance;
    VIBRANCE = "vibrance" => vibrance: || tr!("effects.adjust.vibrance"), AMOUNT;
    SATURATION = "saturation" => saturation: || tr!("effects.adjust.saturation"), AMOUNT;
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vibrance {
    vibrance: f32,
    saturation: f32,
}

impl Parameters for Vibrance {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::Vibrance::new(self.vibrance)
                .saturation(self.saturation)
                .into(),
        )
    }
}
