use super::{HALF_TURN, PERCENT};
use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::tr;
use rikka_image_edit::{self as imaging, Operation};
use serde::{Deserialize, Serialize};

knobs! {
    for TiltShift;
    BLUR = "blur" => blur: || tr!("effects.tilt_shift.blur"), Scale::new(0.0..=500.0)
            .slide(0.0..=100.0)
            .unit(Unit::Pixels);
    CENTER = "center" => center: || tr!("effects.tilt_shift.center"), PERCENT;
    ANGLE = "angle" => angle: || tr!("effects.adjust.angle"), HALF_TURN;
    FOCUS = "focus" => focus: || tr!("effects.tilt_shift.focus"), PERCENT;
    TRANSITION = "transition" => transition: || tr!("effects.tilt_shift.transition"), Scale::new(0.1..=100.0).decimals(1).unit(Unit::Percent);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TiltShift {
    blur: f32,
    center: f32,
    angle: f32,
    focus: f32,
    transition: f32,
}

impl Default for TiltShift {
    fn default() -> TiltShift {
        TiltShift {
            blur: 15.0,
            center: 50.0,
            angle: 0.0,
            focus: 10.0,
            transition: 20.0,
        }
    }
}

impl Parameters for TiltShift {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::TiltShift::new(self.blur)
                .line(self.center / 100.0, self.angle)
                .band(self.focus / 100.0, self.transition / 100.0)
                .into(),
        )
    }

    fn padding(&self) -> f32 {
        self.blur
    }
}
