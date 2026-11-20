use animate_body::{
    Body, Dof, HumanBone, HumanPose, Worlds, muscle_of, muscles, rotation_vector, twist_about,
};
use rikka_model_core::glam::Quat;

use crate::effector::Pin;
use crate::floor::settle;
use crate::handle::Handle;
use crate::hold::{pinned, standing};
use crate::placing::Placing;
use crate::slack::Slack;

fn finish(
    body: &Body,
    pose: &HumanPose,
    moved: HumanPose,
    start: &Worlds,
    handle: Handle,
    pins: &[Pin],
    limits: bool,
) -> HumanPose {
    let mut all_pins = pinned(handle, pins);
    let slackened = Slack::new(body, &all_pins, limits).keep(body, pose, moved);
    all_pins.extend(standing(start, handle, &all_pins));
    settle(body, slackened, &all_pins, pose, limits)
}

#[must_use]
pub fn rotate(
    body: &Body,
    pose: &HumanPose,
    handle: Handle,
    turn: Quat,
    pins: &[Pin],
    limits: bool,
) -> HumanPose {
    let mut placing = Placing::new(body, *pose, pose, limits);
    let start = placing.before().clone();
    if let Some(carrier) = handle.carrier() {
        let end = handle.pivot(body);
        if let Some(axis) = (start.position(end) - start.position(carrier)).try_normalize() {
            let roll = Quat::from_axis_angle(axis, twist_about(turn, axis));
            placing.place(carrier, roll * start.rotation(carrier));
        }
    }
    let bones = handle.turned(body);
    let whole = rotation_vector(turn);
    for (at, bone) in bones.iter().enumerate() {
        let share = Quat::from_scaled_axis(whole * ((at + 1) as f32 / bones.len() as f32));
        placing.place(*bone, share * start.rotation(*bone));
    }
    if handle.keeps_torso() {
        placing.place(HumanBone::Spine, start.rotation(HumanBone::Spine));
    }
    let placed_pose = placing.finish();
    finish(body, pose, placed_pose, &start, handle, pins, limits)
}

#[must_use]
pub fn twist(
    body: &Body,
    pose: &HumanPose,
    handle: Handle,
    angle: f32,
    pins: &[Pin],
    limits: bool,
) -> HumanPose {
    let mut placing = Placing::new(body, *pose, pose, limits);
    let start = placing.before().clone();
    if handle.turns_root() {
        let hips = start.rotation(HumanBone::Hips);
        placing.place(HumanBone::Hips, Quat::from_rotation_y(angle) * hips);
        if handle.keeps_torso() {
            placing.place(HumanBone::Spine, start.rotation(HumanBone::Spine));
        }
    }
    let mut twisted = placing.finish();
    let bones = handle.twisted(body);
    let share = angle / bones.len().max(1) as f32;
    for index in bones
        .into_iter()
        .filter_map(|bone| muscle_of(bone, Dof::Twist))
    {
        let muscle = muscles()[index];
        twisted.muscles[index] = muscle.value(muscle.angle(twisted.muscles[index]) + share);
    }
    body.clamp_around(&mut twisted, pose, limits);
    finish(body, pose, twisted, &start, handle, pins, limits)
}
