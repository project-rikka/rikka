use rikka_model_core::glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegacyHumanoid {
    pub arm_stretch: f32,
    pub leg_stretch: f32,
    pub upper_arm_twist: f32,
    pub lower_arm_twist: f32,
    pub upper_leg_twist: f32,
    pub lower_leg_twist: f32,
    pub feet_spacing: f32,
    pub has_translation_dof: bool,
}

impl Default for LegacyHumanoid {
    fn default() -> Self {
        Self {
            arm_stretch: 0.05,
            leg_stretch: 0.05,
            upper_arm_twist: 0.5,
            lower_arm_twist: 0.5,
            upper_leg_twist: 0.5,
            lower_leg_twist: 0.5,
            feet_spacing: 0.0,
            has_translation_dof: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneLimit {
    pub use_default_values: bool,
    pub min: Vec3,
    pub max: Vec3,
    pub center: Vec3,
    pub axis_length: f32,
}

impl Default for BoneLimit {
    fn default() -> Self {
        Self {
            use_default_values: true,
            min: Vec3::ZERO,
            max: Vec3::ZERO,
            center: Vec3::ZERO,
            axis_length: 0.0,
        }
    }
}
