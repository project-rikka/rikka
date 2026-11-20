use super::TURN;
use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation, Removal};
use serde::{Deserialize, Serialize};

knobs! {
    for SmartSharpen;
    AMOUNT = "amount" => amount: || tr!("effects.adjust.amount"), Scale::new(1.0..=500.0).unit(Unit::Percent);
    RADIUS = "radius" => radius: || tr!("effects.adjust.radius"), Scale::new(0.1..=64.0).decimals(1).unit(Unit::Pixels);
    REDUCE_NOISE = "reduce_noise" => reduce_noise: || tr!("effects.smart_sharpen.reduce_noise"), Scale::new(0.0..=100.0).unit(Unit::Percent);
    ANGLE = "angle" => angle: || tr!("effects.adjust.angle"), TURN;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Remove {
    Gaussian,
    #[default]
    Lens,
    Motion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SmartSharpen {
    amount: f32,
    radius: f32,
    reduce_noise: f32,
    pub remove: Remove,
    angle: f32,
}

impl Default for SmartSharpen {
    fn default() -> SmartSharpen {
        SmartSharpen {
            amount: 200.0,
            radius: 1.0,
            reduce_noise: 10.0,
            remove: Remove::default(),
            angle: 0.0,
        }
    }
}

impl SmartSharpen {
    fn removal(&self) -> Removal {
        match self.remove {
            Remove::Gaussian => Removal::GaussianBlur,
            Remove::Lens => Removal::LensBlur,
            Remove::Motion => Removal::MotionBlur { angle: self.angle },
        }
    }
}

impl Parameters for SmartSharpen {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::SmartSharpen::new(self.amount, self.radius)
                .reduce_noise(self.reduce_noise)
                .removal(self.removal())
                .into(),
        )
    }

    fn padding(&self) -> f32 {
        self.radius * 3.0
    }
}
