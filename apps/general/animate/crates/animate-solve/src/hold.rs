use animate_body::{Body, HumanPose, Side, Worlds};
use rikka_model_core::glam::Vec3;

use crate::effector::Pin;
use crate::handle::Handle;
use crate::two_bone::{Tip, follow};

pub(crate) fn pinned(handle: Handle, pins: &[Pin]) -> Vec<Pin> {
    pins.iter()
        .copied()
        .filter(|pin| Some(pin.limb) != handle.end())
        .collect()
}

pub(crate) fn standing(worlds: &Worlds, handle: Handle, pinned: &[Pin]) -> Vec<Pin> {
    Side::BOTH
        .into_iter()
        .map(Side::leg)
        .filter(|leg| handle.limb() != Some(*leg))
        .filter(|leg| pinned.iter().all(|pin| pin.limb != *leg))
        .map(|leg| Pin::from_worlds(worlds, leg))
        .collect()
}

pub(crate) fn keep(body: &Body, pose: &mut HumanPose, pin: &Pin, around: &HumanPose, limits: bool) {
    if pin.holds(&body.fk(pose)) {
        return;
    }
    let tip = Tip {
        limb: pin.limb,
        grip: Vec3::ZERO,
        goal: pin.position,
        turn: Some(pin.rotation),
    };
    follow(body, pose, tip, around, limits);
}
