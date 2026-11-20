use super::HALF_TURN;
use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for MotionBlur;
    ANGLE = "angle" => angle: || tr!("effects.adjust.angle"), HALF_TURN;
    DISTANCE = "distance" => distance: || tr!("effects.adjust.distance"), Scale::new(1.0..=2000.0)
            .slide(1.0..=500.0)
            .unit(Unit::Pixels);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MotionBlur {
    angle: f32,
    distance: f32,
}

impl Default for MotionBlur {
    fn default() -> MotionBlur {
        MotionBlur {
            angle: 0.0,
            distance: 10.0,
        }
    }
}

impl Parameters for MotionBlur {
    fn operation(&self) -> Option<Operation> {
        Some(imaging::MotionBlur::new(self.angle, self.distance).into())
    }

    fn padding(&self) -> f32 {
        self.distance
    }
}
