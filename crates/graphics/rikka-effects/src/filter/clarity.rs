use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for Clarity;
    AMOUNT = "amount" => amount: || tr!("effects.adjust.amount"), Scale::new(-100.0..=100.0);
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), Scale::new(1.0..=500.0)
            .slide(1.0..=200.0)
            .unit(Unit::Pixels);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clarity {
    amount: f32,
    radius: f32,
}

impl Default for Clarity {
    fn default() -> Clarity {
        Clarity {
            amount: 25.0,
            radius: 50.0,
        }
    }
}

impl Parameters for Clarity {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::Clarity::new(self.amount)
                .radius(self.radius)
                .into(),
        )
    }

    fn padding(&self) -> f32 {
        self.radius * 3.0
    }
}
