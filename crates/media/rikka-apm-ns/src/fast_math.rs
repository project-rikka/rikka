use std::f32::consts::{LN_2, LOG10_E};

fn fast_log2(value: f32) -> f32 {
    let mut out = value.to_bits() as f32;
    out *= 1.192_092_9e-7;
    out -= 126.942_695;
    out
}

pub(crate) fn pow(base: f32, exponent: f32) -> f32 {
    2.0f32.powf(exponent * fast_log2(base))
}

pub(crate) fn log(value: f32) -> f32 {
    fast_log2(value) * LN_2
}

pub(crate) fn exp(value: f32) -> f32 {
    pow(10.0, value * LOG10_E)
}
