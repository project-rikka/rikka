use rikka_model_core::HumanBone;
use rikka_model_core::glam::{Quat, Vec3};

use crate::hierarchy::{HUMAN_BONES, parents};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Joint {
    pub position: Vec3,
    pub rotation: Quat,
}

impl Default for Joint {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

impl Joint {
    #[must_use]
    pub fn point(&self, local: Vec3) -> Vec3 {
        self.position + self.rotation * local
    }

    #[must_use]
    pub fn local(&self, point: Vec3) -> Vec3 {
        self.rotation.inverse() * (point - self.position)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Worlds {
    joints: [Option<Joint>; HUMAN_BONES],
}

impl Default for Worlds {
    fn default() -> Self {
        Self {
            joints: [None; HUMAN_BONES],
        }
    }
}

impl Worlds {
    pub(crate) fn set(&mut self, bone: HumanBone, joint: Joint) {
        self.joints[bone.index()] = Some(joint);
    }

    #[must_use]
    pub fn get(&self, bone: HumanBone) -> Option<Joint> {
        self.joints[bone.index()]
    }

    #[must_use]
    pub fn joint(&self, bone: HumanBone) -> Joint {
        self.get(bone)
            .or_else(|| {
                parents(bone)
                    .into_iter()
                    .find_map(|parent| self.get(parent))
            })
            .or_else(|| self.get(HumanBone::Hips))
            .unwrap_or_default()
    }

    #[must_use]
    pub fn position(&self, bone: HumanBone) -> Vec3 {
        self.joint(bone).position
    }

    #[must_use]
    pub fn rotation(&self, bone: HumanBone) -> Quat {
        self.joint(bone).rotation
    }

    #[must_use]
    pub fn point(&self, bone: HumanBone, local: Vec3) -> Vec3 {
        self.joint(bone).point(local)
    }

    pub fn iter(&self) -> impl Iterator<Item = (HumanBone, Joint)> + '_ {
        HumanBone::ALL
            .iter()
            .filter_map(|bone| Some((*bone, self.get(*bone)?)))
    }
}
