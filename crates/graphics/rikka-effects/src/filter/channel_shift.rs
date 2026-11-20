use super::TURN;
use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for ChannelShift;
    ANGLE = "angle" => angle: || tr!("effects.adjust.angle"), TURN;
    DISTANCE = "distance" => distance: || tr!("effects.adjust.distance"), Scale::new(0.0..=2000.0)
            .slide(0.0..=100.0)
            .decimals(1)
            .unit(Unit::Pixels);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelShift {
    angle: f32,
    distance: f32,
}

impl Default for ChannelShift {
    fn default() -> ChannelShift {
        ChannelShift {
            angle: 0.0,
            distance: 4.0,
        }
    }
}

impl Parameters for ChannelShift {
    fn operation(&self) -> Option<Operation> {
        Some(imaging::ChannelShift::new(self.angle, self.distance).into())
    }

    fn padding(&self) -> f32 {
        self.distance
    }
}
