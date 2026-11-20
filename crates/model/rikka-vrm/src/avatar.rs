use rikka_model_core::Conversion;
use rikka_model_core::Extension;
use rikka_model_core::Model;
use rikka_model_core::NodeId;
use rikka_model_core::Rig;
use rikka_model_core::{Follow, RigFacts};
use rikka_model_core::{HumanBone, Humanoid};

use super::constraint::NodeConstraint;
use super::expression::{Expression, ExpressionName};
use super::first_person::MeshAnnotation;
use super::look_at::LookAt;
use super::meta::Meta;
use super::runtime::Avatar;
use super::spring::SpringBone;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VrmVersion {
    Legacy,
    #[default]
    V1,
}

impl VrmVersion {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "0.x",
            Self::V1 => "1.0",
        }
    }
}

impl std::fmt::Display for VrmVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Vrm {
    pub version: VrmVersion,
    pub meta: Meta,
    pub humanoid: Humanoid,
    pub first_person: Vec<MeshAnnotation>,
    pub look_at: LookAt,
    pub expressions: Vec<Expression>,
    pub spring_bone: SpringBone,
    pub constraints: Vec<NodeConstraint>,
}

impl Extension for Vrm {
    const NAME: &'static str = "VRMC_vrm";

    fn convert(&mut self, conversion: &Conversion) {
        self.look_at.convert(conversion);
        self.spring_bone.convert(conversion);
        for node_constraint in &mut self.constraints {
            node_constraint.constraint.convert(conversion);
        }
    }

    fn rig(&self, model: &Model) -> Option<Box<dyn Rig>> {
        Avatar::new(model).map(|avatar| Box::new(avatar) as Box<dyn Rig>)
    }

    fn describe_rig(&self, facts: &mut RigFacts) {
        if !self.humanoid.is_empty() {
            facts.humanoid = Some(self.humanoid.clone());
        }
        for spring in &self.spring_bone.springs {
            facts
                .swinging
                .extend(spring.joints.iter().map(|joint| joint.node));
        }
        for node_constraint in &self.constraints {
            facts.follows.push(Follow {
                node: node_constraint.node,
                source: node_constraint.constraint.source(),
                ratio: node_constraint.constraint.weight(),
            });
        }
    }
}

impl Vrm {
    #[must_use]
    pub fn bone(&self, bone: HumanBone) -> Option<NodeId> {
        self.humanoid.bone(bone)
    }

    #[must_use]
    pub fn expression(&self, name: &ExpressionName) -> Option<&Expression> {
        self.expressions
            .iter()
            .find(|expression| &expression.name == name)
    }

    #[must_use]
    pub fn expression_index(&self, name: &ExpressionName) -> Option<usize> {
        self.expressions
            .iter()
            .position(|expression| &expression.name == name)
    }
}
