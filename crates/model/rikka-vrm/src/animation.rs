use rikka_model_core::glam::{EulerRot, Vec3};
use rikka_model_core::{Conversion, Extension, Humanoid, NodeId, Pose, RigFacts};

use super::expression::ExpressionName;

#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedExpression {
    pub name: ExpressionName,
    pub node: NodeId,
}

impl AnimatedExpression {
    #[must_use]
    pub fn weight(&self, pose: &Pose) -> f32 {
        pose.local(self.node).translation.x.clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimatedLookAt {
    pub node: Option<NodeId>,
    pub offset_from_head: Option<Vec3>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gaze {
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VrmAnimation {
    pub humanoid: Humanoid,
    pub expressions: Vec<AnimatedExpression>,
    pub look_at: Option<AnimatedLookAt>,
}

impl VrmAnimation {
    pub fn expression_weights<'a>(
        &'a self,
        pose: &'a Pose,
    ) -> impl Iterator<Item = (&'a ExpressionName, f32)> + 'a {
        self.expressions
            .iter()
            .map(|expression| (&expression.name, expression.weight(pose)))
    }

    #[must_use]
    pub fn gaze(&self, pose: &Pose) -> Option<Gaze> {
        let node = self.look_at?.node?;
        let (yaw, pitch, _) = pose.local(node).rotation.to_euler(EulerRot::YXZ);
        Some(Gaze { yaw, pitch })
    }
}

impl Extension for VrmAnimation {
    const NAME: &'static str = "VRMC_vrm_animation";

    fn convert(&mut self, conversion: &Conversion) {
        if let Some(offset) = self
            .look_at
            .as_mut()
            .and_then(|look_at| look_at.offset_from_head.as_mut())
        {
            *offset = conversion.point(*offset);
        }
    }

    fn describe_rig(&self, facts: &mut RigFacts) {
        if !self.humanoid.is_empty() {
            facts.humanoid = Some(self.humanoid.clone());
        }
    }
}
