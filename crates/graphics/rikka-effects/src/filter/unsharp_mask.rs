use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for UnsharpMask;
    AMOUNT = "amount" => amount: || tr!("effects.adjust.amount"), Scale::new(1.0..=500.0).unit(Unit::Percent);
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), crate::filter::RADIUS;
    THRESHOLD = "threshold" => threshold: || tr!("effects.adjust.threshold_levels"), Scale::new(0.0..=255.0);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnsharpMask {
    amount: f32,
    radius: f32,
    threshold: f32,
}

impl Default for UnsharpMask {
    fn default() -> UnsharpMask {
        UnsharpMask {
            amount: 100.0,
            radius: 1.0,
            threshold: 0.0,
        }
    }
}

impl Parameters for UnsharpMask {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::UnsharpMask::new(self.amount, self.radius)
                .threshold(self.threshold)
                .into(),
        )
    }

    fn padding(&self) -> f32 {
        self.radius * 3.0
    }
}
