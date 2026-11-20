use super::TURN;
use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for Emboss;
    ANGLE = "angle" => angle: || tr!("effects.adjust.angle"), TURN;
    HEIGHT = "height" => height: || tr!("effects.emboss.height"), Scale::new(1.0..=100.0).unit(Unit::Pixels);
    AMOUNT = "amount" => amount: || tr!("effects.adjust.amount"), Scale::new(1.0..=500.0).unit(Unit::Percent);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Emboss {
    angle: f32,
    height: f32,
    amount: f32,
}

impl Default for Emboss {
    fn default() -> Emboss {
        Emboss {
            angle: 135.0,
            height: 3.0,
            amount: 100.0,
        }
    }
}

impl Parameters for Emboss {
    fn operation(&self) -> Option<Operation> {
        Some(imaging::Emboss::new(self.angle, self.height, self.amount).into())
    }

    fn padding(&self) -> f32 {
        self.height
    }
}
