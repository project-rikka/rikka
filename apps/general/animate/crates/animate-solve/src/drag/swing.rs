use animate_body::{Dof, HumanBone, HumanPose, arc, muscle_of, muscles};
use rikka_model_core::glam::{Quat, Vec3};

use crate::drag::Start;
use crate::placing::Placing;
use crate::turns::{across, is_short, signed_angle};

pub(super) fn swing(start: &Start<'_>, pose: HumanPose, target: Vec3) -> HumanPose {
    let body = start.body;
    let bone = start.handle.pivot(body);
    let mut placing = Placing::new(body, pose, start.pose, start.limits);
    let worlds = placing.before();
    let pivot = worlds.position(bone);
    let grip = start.handle.point(body, worlds);
    let desired = arc(grip - pivot, target - pivot) * worlds.rotation(bone);
    placing.swing(bone, desired);
    placing.finish()
}

pub(super) fn pelvis(start: &Start<'_>, pose: HumanPose, target: Vec3) -> HumanPose {
    let body = start.body;
    let mut placing = Placing::new(body, pose, start.pose, start.limits);
    let worlds = placing.before();
    let hips = worlds.position(HumanBone::Hips);
    let grip = start.handle.point(body, worlds);
    let rotation = arc(grip - hips, target - hips) * worlds.rotation(HumanBone::Hips);
    let spine = worlds.rotation(HumanBone::Spine);
    placing.place(HumanBone::Hips, rotation);
    placing.place(HumanBone::Spine, spine);
    placing.finish()
}

pub(super) fn hinge(start: &Start<'_>, mut pose: HumanPose, target: Vec3) -> HumanPose {
    let body = start.body;
    let Some(limb) = start.handle.limb() else {
        return pose;
    };
    let [upper, lower, _] = limb.chain();
    let (Some(axis), Some(index)) = (body.axis(lower, Dof::Bend), muscle_of(lower, Dof::Bend))
    else {
        return pose;
    };
    let worlds = body.fk(&pose);
    let axis = worlds.rotation(upper) * axis;
    let joint = worlds.position(lower);
    let from = across(start.handle.point(body, &worlds) - joint, axis);
    let to = across(target - joint, axis);
    if is_short(from) || is_short(to) {
        return pose;
    }
    let muscle = muscles()[index];
    let angle = muscle.angle(pose.muscles[index]) + signed_angle(from, to, axis);
    pose.muscles[index] = muscle.value(angle);
    body.clamp_around(&mut pose, start.pose, start.limits);
    pose
}

pub(super) fn swivel(start: &Start<'_>, pose: HumanPose, target: Vec3, pin: Vec3) -> HumanPose {
    let body = start.body;
    let Some(limb) = start.handle.limb() else {
        return pose;
    };
    let upper = limb.chain()[0];
    let mut placing = Placing::new(body, pose, start.pose, start.limits);
    let worlds = placing.before();
    let root = worlds.position(upper);
    let Some(axis) = (pin - root).try_normalize() else {
        return pose;
    };
    let from = across(start.handle.point(body, worlds) - root, axis);
    let to = across(target - root, axis);
    if is_short(from) || is_short(to) {
        return pose;
    }
    let desired =
        Quat::from_axis_angle(axis, signed_angle(from, to, axis)) * worlds.rotation(upper);
    placing.place(upper, desired);
    placing.finish()
}
