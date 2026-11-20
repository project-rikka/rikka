use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub(crate) const VRM: &str = "VRMC_vrm";

pub(crate) const SPRING_BONE: &str = "VRMC_springBone";

pub(crate) const EXTENDED_COLLIDER: &str = "VRMC_springBone_extended_collider";

pub(crate) const NODE_CONSTRAINT: &str = "VRMC_node_constraint";

pub(crate) const MTOON: &str = "VRMC_materials_mtoon";

pub(crate) const SPEC_VERSION: &str = "1.0";

pub(crate) const VRM_ANIMATION: &str = "VRMC_vrm_animation";

type Index = u32;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VrmAnimation {
    pub spec_version: String,
    #[serde(default)]
    pub humanoid: Option<Humanoid>,
    #[serde(default)]
    pub expressions: Option<AnimatedExpressions>,
    #[serde(default)]
    pub look_at: Option<AnimatedLookAt>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnimatedExpressions {
    #[serde(default)]
    pub preset: BTreeMap<String, AnimatedNode>,
    #[serde(default)]
    pub custom: BTreeMap<String, AnimatedNode>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnimatedNode {
    pub node: Index,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnimatedLookAt {
    #[serde(default)]
    pub node: Option<Index>,
    #[serde(default)]
    pub offset_from_head_bone: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Vrm {
    pub spec_version: String,
    pub meta: Meta,
    pub humanoid: Humanoid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_person: Option<FirstPerson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_at: Option<LookAt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expressions: Option<Expressions>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Meta {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright_information: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_information: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub third_party_licenses: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_image: Option<Index>,
    pub license_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other_license_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_permission: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commercial_usage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_notation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_redistribution: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_excessively_violent_usage: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_excessively_sexual_usage: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_political_or_religious_usage: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_antisocial_or_hate_usage: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Humanoid {
    pub human_bones: BTreeMap<String, HumanBone>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HumanBone {
    pub node: Index,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FirstPerson {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mesh_annotations: Vec<MeshAnnotation>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeshAnnotation {
    pub node: Index,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LookAt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_from_head_bone: Option<[f32; 3]>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_map_horizontal_inner: Option<RangeMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_map_horizontal_outer: Option<RangeMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_map_vertical_down: Option<RangeMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_map_vertical_up: Option<RangeMap>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RangeMap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_max_value: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_scale: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Expressions {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub preset: BTreeMap<String, Expression>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom: BTreeMap<String, Expression>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Expression {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morph_target_binds: Vec<MorphTargetBind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material_color_binds: Vec<MaterialColorBind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub texture_transform_binds: Vec<TextureTransformBind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_binary: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_blink: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_look_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_mouth: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MorphTargetBind {
    pub node: Index,
    pub index: Index,
    pub weight: f32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MaterialColorBind {
    pub material: Index,
    #[serde(rename = "type")]
    pub kind: String,
    pub target_value: [f32; 4],
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextureTransformBind {
    pub material: Index,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 2]>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpringBone {
    pub spec_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colliders: Vec<Collider>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collider_groups: Vec<ColliderGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub springs: Vec<Spring>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Collider {
    pub node: Index,
    pub shape: ColliderShape,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<ColliderExtensions>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct ColliderExtensions {
    #[serde(
        rename = "VRMC_springBone_extended_collider",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub extended_collider: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtendedCollider {
    pub spec_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<ExtendedShape>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtendedShape {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sphere: Option<ExtendedSphere>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capsule: Option<ExtendedCapsule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane: Option<ExtendedPlane>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtendedSphere {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inside: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtendedCapsule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inside: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtendedPlane {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ColliderShape {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sphere: Option<Sphere>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capsule: Option<Capsule>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Sphere {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Capsule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ColliderGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub colliders: Vec<Index>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Spring {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub joints: Vec<Joint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collider_groups: Vec<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<Index>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Joint {
    pub node: Index,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stiffness: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity_power: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity_dir: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_force: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeConstraint {
    pub spec_version: String,
    pub constraint: Constraint,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Constraint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roll: Option<RollConstraint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aim: Option<AimConstraint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<RotationConstraint>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RollConstraint {
    pub source: Index,
    pub roll_axis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AimConstraint {
    pub source: Index,
    pub aim_axis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RotationConstraint {
    pub source: Index,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MToon {
    pub spec_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent_with_z_write: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_queue_offset_number: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shade_color_factor: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shade_multiply_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shading_shift_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shading_shift_texture: Option<ShadingShiftTexture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shading_toony_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gi_equalization_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcap_factor: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcap_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parametric_rim_color_factor: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rim_multiply_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rim_lighting_mix_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parametric_rim_fresnel_power_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parametric_rim_lift_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_width_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_width_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_width_multiply_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_color_factor: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_lighting_mix_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_animation_mask_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_animation_scroll_x_speed_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_animation_scroll_y_speed_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_animation_rotation_speed_factor: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextureInfo {
    pub index: Index,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tex_coord: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShadingShiftTexture {
    pub index: Index,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tex_coord: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}
