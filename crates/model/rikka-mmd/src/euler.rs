use rikka_model_core::glam::{Quat, Vec3};

use rikka_model_core::{Conversion, EulerOrder};

#[must_use]
pub(crate) fn rotation_of(angles: Vec3) -> Quat {
    EulerOrder::Zxy.quaternion(angles)
}

#[must_use]
pub(crate) fn angles_of(rotation: Quat) -> Vec3 {
    EulerOrder::Zxy.angles(rotation)
}

pub(crate) fn point_bounds(conversion: &Conversion, lower: Vec3, upper: Vec3) -> (Vec3, Vec3) {
    ordered(lower, upper, |axis| {
        let (target, factor) = image(conversion, axis);
        (target, factor * conversion.scale())
    })
}

pub(crate) fn angle_bounds(conversion: &Conversion, lower: Vec3, upper: Vec3) -> (Vec3, Vec3) {
    ordered(lower, upper, |axis| carried(conversion, axis))
}

fn ordered(lower: Vec3, upper: Vec3, carry: impl Fn(usize) -> (usize, f32)) -> (Vec3, Vec3) {
    let mut low = Vec3::ZERO;
    let mut high = Vec3::ZERO;
    for axis in 0..3 {
        let (target, factor) = carry(axis);
        let (lower, upper) = (lower[axis] * factor, upper[axis] * factor);
        (low[target], high[target]) = match factor < 0.0 {
            true => (upper, lower),
            false => (lower, upper),
        };
    }
    (low, high)
}

#[must_use]
pub(crate) fn angles(conversion: &Conversion, angles: Vec3) -> Vec3 {
    let mut turned = Vec3::ZERO;
    for axis in 0..3 {
        let (target, factor) = carried(conversion, axis);
        turned[target] = angles[axis] * factor;
    }
    turned
}

fn carried(conversion: &Conversion, axis: usize) -> (usize, f32) {
    let (target, sign) = image(conversion, axis);
    let handedness = match conversion.flips_winding() {
        true => -1.0,
        false => 1.0,
    };
    (target, sign * handedness)
}

fn image(conversion: &Conversion, axis: usize) -> (usize, f32) {
    let mut source = Vec3::ZERO;
    source[axis] = 1.0;
    let turned = conversion.vector(source);
    let target = match turned.abs().to_array() {
        [x, y, z] if x >= y && x >= z => 0,
        [_, y, z] if y >= z => 1,
        _ => 2,
    };
    (target, turned[target].signum())
}
