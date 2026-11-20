use animate_body::{Body, HumanBone, HumanPose, Limb, Worlds};
use rikka_model_core::glam::{Quat, Vec3};

use crate::extent::Extent;
use crate::tuning::{PIN_SLACK, PIN_TURN_SLACK, WEIGHT_PIN};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pin {
    pub limb: Limb,
    pub position: Vec3,
    pub rotation: Quat,
}

impl Pin {
    #[must_use]
    pub fn from_pose(body: &Body, pose: &HumanPose, limb: Limb) -> Self {
        Self::from_worlds(&body.fk(pose), limb)
    }

    pub(crate) fn from_worlds(worlds: &Worlds, limb: Limb) -> Self {
        let joint = worlds.joint(limb.chain()[2]);
        Self {
            limb,
            position: joint.position,
            rotation: joint.rotation,
        }
    }

    pub(crate) fn holds(&self, worlds: &Worlds) -> bool {
        let joint = worlds.joint(self.limb.chain()[2]);
        joint.position.distance(self.position) <= PIN_SLACK
            && joint.rotation.angle_between(self.rotation) <= PIN_TURN_SLACK
    }

    pub(crate) fn clamped(&self, body: &Body, limits: bool) -> Goal {
        Goal::Within {
            bone: self.limb.chain()[0],
            target: self.position,
            radius: Extent::measure(body, self.limb, limits).far,
            weight: WEIGHT_PIN,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Goal {
    Place {
        bone: HumanBone,
        local: Vec3,
        target: Vec3,
    },
    Within {
        bone: HumanBone,
        target: Vec3,
        radius: f32,
        weight: f32,
    },
}

impl Goal {
    pub(crate) fn retarget(&mut self, aim: Vec3) {
        match self {
            Self::Place { target, .. } | Self::Within { target, .. } => *target = aim,
        }
    }

    pub(crate) fn residual(&self, worlds: &Worlds) -> Vec3 {
        match *self {
            Self::Place {
                bone,
                local,
                target,
            } => target - worlds.point(bone, local),
            Self::Within {
                bone,
                target,
                radius,
                weight,
            } => {
                let apart = target - worlds.position(bone);
                let beyond = apart.length() - radius;
                match beyond > 0.0 {
                    true => apart.normalize_or_zero() * (beyond * weight.sqrt()),
                    false => Vec3::ZERO,
                }
            }
        }
    }
}
