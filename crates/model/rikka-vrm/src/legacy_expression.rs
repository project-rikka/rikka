use rikka_model_core::MaterialId;

use super::expression::ExpressionName;

#[derive(Clone, Debug, PartialEq)]
pub struct LegacyMaterialValue {
    pub material: MaterialId,
    pub property: String,
    pub target_value: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LegacyExpression {
    pub name: ExpressionName,
    pub group_name: String,
    pub material_values: Vec<LegacyMaterialValue>,
}

impl LegacyExpression {
    pub fn new(name: impl Into<ExpressionName>, group_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            group_name: group_name.into(),
            material_values: Vec::new(),
        }
    }
}
