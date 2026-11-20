use std::f32::consts::PI;

use crate::codec::{record, within};

#[derive(Clone, Debug, PartialEq)]
pub struct SmoothByAngle {
    pub angle: f32,
    pub ignore_sharpness: bool,
}

impl Default for SmoothByAngle {
    fn default() -> SmoothByAngle {
        SmoothByAngle {
            angle: 30.0f32.to_radians(),
            ignore_sharpness: false,
        }
    }
}

record!(SmoothByAngle {
    angle => within(0.0, PI, "smooth by angle is outside 0..180 degrees"),
    ignore_sharpness,
});
