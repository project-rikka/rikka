use crate::hex;
use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use rikka_types::Color;
use serde::{Deserialize, Serialize};

const WIDTH: Scale = Scale::new(0.0..=1000.0)
    .slide(0.0..=50.0)
    .decimals(1)
    .unit(Unit::Pixels);

knobs! {
    for Outline;
    WIDTH_KNOB = "width" => width: || tr!("effects.outline.width"), WIDTH;
    SOFTNESS = "softness" => softness: || tr!("effects.key.softness"), WIDTH;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Outline {
    #[serde(with = "hex")]
    pub color: Color,
    width: f32,
    softness: f32,
}

impl Default for Outline {
    fn default() -> Outline {
        Outline {
            color: Color::BLACK,
            width: 4.0,
            softness: 0.0,
        }
    }
}

impl Parameters for Outline {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::Outline::new(self.color)
                .width(self.width)
                .softness(self.softness)
                .into(),
        )
    }

    fn padding(&self) -> f32 {
        self.width + self.softness / 2.0
    }
}
