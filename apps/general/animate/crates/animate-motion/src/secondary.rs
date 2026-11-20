use std::f32::consts::TAU;

use animate_body::{Body, HumanBone, HumanPose, Limb, muscles_of, rotation_vector};
use rikka_model_core::glam::{Quat, Vec3};

use crate::sample::Pinned;
use crate::tuning::{
    FRAME_SECONDS, SECONDARY_DAMPING, SECONDARY_REACH_DEGREES, SECONDARY_SOFT_HZ,
    SECONDARY_STIFF_HZ,
};

const BONES: usize = 55;
const LIMIT: f32 = 1.0;

const TORSO: [HumanBone; 4] = [
    HumanBone::Chest,
    HumanBone::UpperChest,
    HumanBone::Neck,
    HumanBone::Head,
];

const LEFT_ARM: [HumanBone; 3] = [
    HumanBone::LeftUpperArm,
    HumanBone::LeftLowerArm,
    HumanBone::LeftHand,
];

const RIGHT_ARM: [HumanBone; 3] = [
    HumanBone::RightUpperArm,
    HumanBone::RightLowerArm,
    HumanBone::RightHand,
];

const LEFT_FINGERS: [HumanBone; 5] = [
    HumanBone::LeftThumbMetacarpal,
    HumanBone::LeftIndexProximal,
    HumanBone::LeftMiddleProximal,
    HumanBone::LeftRingProximal,
    HumanBone::LeftLittleProximal,
];

const RIGHT_FINGERS: [HumanBone; 5] = [
    HumanBone::RightThumbMetacarpal,
    HumanBone::RightIndexProximal,
    HumanBone::RightMiddleProximal,
    HumanBone::RightRingProximal,
    HumanBone::RightLittleProximal,
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Spring {
    turn: Quat,
    spin: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Springs {
    bones: [Option<Spring>; BONES],
}

impl Default for Springs {
    fn default() -> Self {
        Self {
            bones: [None; BONES],
        }
    }
}

struct Chain<'a> {
    bones: &'a [HumanBone],
    serial: bool,
    held_by: Option<Limb>,
}

const CHAINS: [Chain<'static>; 5] = [
    Chain {
        bones: &TORSO,
        serial: true,
        held_by: None,
    },
    Chain {
        bones: &LEFT_ARM,
        serial: true,
        held_by: Some(Limb::LeftArm),
    },
    Chain {
        bones: &RIGHT_ARM,
        serial: true,
        held_by: Some(Limb::RightArm),
    },
    Chain {
        bones: &LEFT_FINGERS,
        serial: false,
        held_by: None,
    },
    Chain {
        bones: &RIGHT_FINGERS,
        serial: false,
        held_by: None,
    },
];

impl Springs {
    pub(crate) fn clear(&mut self) {
        self.bones = [None; BONES];
    }

    pub(crate) fn lag(
        &mut self,
        body: &Body,
        pose: &mut HumanPose,
        pins: &[Pinned; 4],
        strength: f32,
    ) {
        let strength = strength.clamp(0.0, 1.0);
        if strength <= 0.0 {
            self.clear();
            return;
        }
        let pitch =
            TAU * (SECONDARY_STIFF_HZ + (SECONDARY_SOFT_HZ - SECONDARY_STIFF_HZ) * strength);
        let pose_sample = *pose;
        for chain in &CHAINS {
            let pinned = chain
                .held_by
                .is_some_and(|limb| pins[limb.index()].weight > 0.0);
            let worlds = body.fk(pose);
            let mut above: Option<Quat> = None;
            for bone in chain.bones {
                let slot = bone.index();
                let Some(parent) = body.parent(*bone).filter(|_| body.has(*bone)) else {
                    self.bones[slot] = None;
                    continue;
                };
                let carried = match (chain.serial, above) {
                    (true, Some(turn)) => turn,
                    _ => worlds.rotation(parent),
                };
                let target = (carried * body.local_rotation(*bone, &pose_sample)).normalize();
                if pinned {
                    self.bones[slot] = None;
                    above = Some(target);
                    continue;
                }
                let spring = self.bones[slot].unwrap_or(Spring {
                    turn: target,
                    spin: Vec3::ZERO,
                });
                let gap = rotation_vector(spring.turn * target.inverse());
                let (offset, spin) = swing(gap, spring.spin, pitch);
                let desired = (Quat::from_scaled_axis(offset) * target).normalize();
                body.set_local_rotation(*bone, carried.inverse() * desired, pose);
                for index in muscles_of(*bone) {
                    let low = pose_sample.muscles[index].min(-LIMIT);
                    let high = pose_sample.muscles[index].max(LIMIT);
                    pose.muscles[index] = pose.muscles[index].clamp(low, high);
                }
                let reached = (carried * body.local_rotation(*bone, pose)).normalize();
                self.bones[slot] = Some(Spring {
                    turn: reached,
                    spin,
                });
                above = Some(reached);
            }
        }
    }
}

fn swing(offset: Vec3, spin: Vec3, pitch: f32) -> (Vec3, Vec3) {
    let damping = SECONDARY_DAMPING;
    let ringing = pitch * (1.0 - damping * damping).sqrt();
    let decay = (-damping * pitch * FRAME_SECONDS).exp();
    let (sine, cosine) = (ringing * FRAME_SECONDS).sin_cos();
    let moved = decay * (offset * cosine + (spin + offset * damping * pitch) / ringing * sine);
    let spun = decay
        * (spin * cosine - (offset * pitch * pitch + spin * damping * pitch) / ringing * sine);
    let reach = SECONDARY_REACH_DEGREES.to_radians();
    match moved.length() > reach {
        true => (moved.normalize() * reach, spun - spun.project_onto(moved)),
        false => (moved, spun),
    }
}
