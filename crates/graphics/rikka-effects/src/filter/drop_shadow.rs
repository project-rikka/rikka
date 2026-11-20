use super::{PERCENT, TURN};
use crate::hex;
use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use rikka_types::Color;
use serde::{Deserialize, Serialize};

knobs! {
    for DropShadow;
    ANGLE = "angle" => angle: || tr!("effects.adjust.angle"), TURN;
    DISTANCE = "distance" => distance: || tr!("effects.adjust.distance"), Scale::new(0.0..=2000.0)
            .slide(0.0..=200.0)
            .decimals(1)
            .unit(Unit::Pixels);
    BLUR = "blur" => blur: || tr!("effects.drop_shadow.blur"), Scale::new(0.0..=1000.0)
            .slide(0.0..=100.0)
            .decimals(1)
            .unit(Unit::Pixels);
    OPACITY = "opacity" => opacity: || tr!("effects.drop_shadow.opacity"), PERCENT;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DropShadow {
    #[serde(with = "hex")]
    pub color: Color,
    angle: f32,
    distance: f32,
    blur: f32,
    opacity: f32,
}

impl Default for DropShadow {
    fn default() -> DropShadow {
        DropShadow {
            color: Color::BLACK,
            angle: -45.0,
            distance: 8.0,
            blur: 4.0,
            opacity: 75.0,
        }
    }
}

impl Parameters for DropShadow {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::DropShadow::new(self.color)
                .angle(self.angle)
                .distance(self.distance)
                .blur(self.blur)
                .opacity(self.opacity / 100.0)
                .into(),
        )
    }

    fn padding(&self) -> f32 {
        self.distance + self.blur * 3.0
    }
}
