use animate_body::{Body, HumanBone, Worlds};
use rikka_model_core::glam::{Quat, Vec3};

const SHORT: f32 = 1e-6;

pub(crate) fn across(vector: Vec3, axis: Vec3) -> Vec3 {
    vector - axis * vector.dot(axis)
}

pub(crate) fn signed_angle(from: Vec3, to: Vec3, axis: Vec3) -> f32 {
    from.cross(to).dot(axis).atan2(from.dot(to))
}

pub(crate) fn is_short(vector: Vec3) -> bool {
    vector.length() <= SHORT
}

pub(crate) fn parent_turn(body: &Body, worlds: &Worlds, bone: HumanBone) -> Quat {
    body.parent(bone)
        .map_or(Quat::IDENTITY, |parent| worlds.rotation(parent))
}
