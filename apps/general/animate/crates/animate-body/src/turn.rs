use rikka_model_core::glam::{Quat, Vec3};

#[must_use]
pub fn canonical(rotation: Quat) -> Quat {
    let unit = rotation.normalize();
    match unit.w < 0.0 {
        true => -unit,
        false => unit,
    }
}

#[must_use]
pub fn twist_about(rotation: Quat, axis: Vec3) -> f32 {
    let shortest = canonical(rotation);
    2.0 * shortest.xyz().dot(axis).atan2(shortest.w)
}

#[must_use]
pub fn arc(from: Vec3, to: Vec3) -> Quat {
    match (from.try_normalize(), to.try_normalize()) {
        (Some(from), Some(to)) => Quat::from_rotation_arc(from, to),
        _ => Quat::IDENTITY,
    }
}

#[must_use]
pub fn rotation_vector(rotation: Quat) -> Vec3 {
    canonical(rotation).to_scaled_axis()
}

pub(crate) fn split(rotation: Quat, axis: Vec3) -> (Quat, f32) {
    let shortest = canonical(rotation);
    let angle = twist_about(shortest, axis);
    let twist = Quat::from_axis_angle(axis, angle);
    (canonical(shortest * twist.inverse()), angle)
}
