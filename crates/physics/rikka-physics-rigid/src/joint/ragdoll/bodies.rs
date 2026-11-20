use super::bone::HumanBone;
use crate::{BodyId, JointId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RagdollBodies {
    pub(crate) bodies: Vec<BodyId>,
    pub(crate) joints: Vec<Option<JointId>>,
    pub(crate) filters: Vec<JointId>,
}

impl RagdollBodies {
    pub fn body(&self, bone: HumanBone) -> BodyId {
        self.bodies[bone.index()]
    }

    pub fn joint(&self, bone: HumanBone) -> Option<JointId> {
        self.joints[bone.index()]
    }

    pub fn bodies(&self) -> &[BodyId] {
        &self.bodies
    }

    pub fn joints(&self) -> impl Iterator<Item = JointId> + '_ {
        self.joints.iter().flatten().chain(&self.filters).copied()
    }
}
