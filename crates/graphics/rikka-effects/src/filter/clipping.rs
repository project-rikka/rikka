use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

const BAND: Scale = Scale::new(0.0..=100_000.0)
    .slide(0.0..=1000.0)
    .unit(Unit::Pixels);

knobs! {
    for Clipping;
    LEFT = "left" => left: || tr!("effects.clipping.left"), BAND;
    TOP = "top" => top: || tr!("effects.clipping.top"), BAND;
    RIGHT = "right" => right: || tr!("effects.clipping.right"), BAND;
    BOTTOM = "bottom" => bottom: || tr!("effects.clipping.bottom"), BAND;
    FEATHER = "feather" => feather: || tr!("effects.clipping.feather"), Scale::new(0.0..=1000.0)
            .slide(0.0..=100.0)
            .decimals(1)
            .unit(Unit::Pixels);
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clipping {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    feather: f32,
}

impl Parameters for Clipping {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::Clipping::new(self.left, self.top, self.right, self.bottom)
                .feather(self.feather)
                .into(),
        )
    }
}
