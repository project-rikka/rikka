use std::f32::consts::{PI, TAU};

use rikka_simd::{FloatMath, Vector};

use crate::F32x1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CosSin {
    pub cosine: f32,
    pub sine: f32,
}

#[inline]
pub fn unwind_angle(radians: f32) -> f32 {
    if radians < -PI {
        radians + TAU * ((PI - radians) / TAU).floor()
    } else if radians > PI {
        radians - TAU * ((radians + PI) / TAU).floor()
    } else {
        radians
    }
}

pub fn cos_sin(radians: f32) -> CosSin {
    let (sine, cosine) = F32x1::splat(radians).sin_cos();
    CosSin {
        cosine: cosine.lane(0),
        sine: sine.lane(0),
    }
}

pub fn atan2(y: f32, x: f32) -> f32 {
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }
    F32x1::splat(y).atan2(F32x1::splat(x)).lane(0)
}
