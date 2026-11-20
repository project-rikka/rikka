use std::collections::BTreeMap;

use rikka_model_core::Extension;
use rikka_model_core::HumanBone;
use rikka_model_core::NodeId;

use super::expression::ExpressionName;
use super::legacy_expression::LegacyExpression;
use super::legacy_humanoid::{BoneLimit, LegacyHumanoid};
use super::license::LegacyLicense;
use super::look_at_curves::LookAtCurves;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VrmLegacy {
    pub license: LegacyLicense,
    pub humanoid: LegacyHumanoid,
    pub bone_limits: BTreeMap<HumanBone, BoneLimit>,
    pub first_person_bone: Option<NodeId>,
    pub look_at_curves: LookAtCurves,
    pub expressions: Vec<LegacyExpression>,
}

impl Extension for VrmLegacy {
    const NAME: &'static str = "VRM";
}

impl VrmLegacy {
    #[must_use]
    pub fn expression(&self, name: &ExpressionName) -> Option<&LegacyExpression> {
        self.expressions
            .iter()
            .find(|expression| &expression.name == name)
    }
}
