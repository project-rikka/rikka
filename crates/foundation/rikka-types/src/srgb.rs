pub const LUMINANCE: [f32; 3] = [0.2126, 0.7152, 0.0722];

#[must_use]
pub fn to_linear(encoded: f32) -> f32 {
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

#[must_use]
pub fn from_linear(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

#[must_use]
pub fn to_linear_f64(encoded: f64) -> f64 {
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

#[must_use]
pub fn from_linear_f64(linear: f64) -> f64 {
    if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

#[must_use]
pub fn luminance([r, g, b]: [f32; 3]) -> f32 {
    LUMINANCE[0] * r + LUMINANCE[1] * g + LUMINANCE[2] * b
}
