use std::collections::BTreeMap;

use rikka_model_core::HumanBone as Bone;
use rikka_model_core::glam::Vec3;
use rikka_model_core::{Axis, CoordinateSystem};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::expression::ExpressionPreset;

pub(crate) const VRM: &str = "VRM";

pub(crate) const SPEC_VERSION: &str = "0.0";

pub(crate) const COORDINATES: CoordinateSystem = CoordinateSystem {
    right: Axis::NegX,
    up: Axis::Y,
    forward: Axis::Z,
};

pub(crate) const OUTLINE_WORLD_SCALE: f32 = 0.01;

pub(crate) const OUTLINE_SCREEN_SCALE: f32 = 0.005;

type Index = u32;

pub(crate) fn bone_from_name(name: &str) -> Option<Bone> {
    match name {
        "leftThumbProximal" => Some(Bone::LeftThumbMetacarpal),
        "leftThumbIntermediate" => Some(Bone::LeftThumbProximal),
        "rightThumbProximal" => Some(Bone::RightThumbMetacarpal),
        "rightThumbIntermediate" => Some(Bone::RightThumbProximal),
        _ => Bone::from_name(name),
    }
}

pub(crate) fn bone_name(bone: Bone) -> &'static str {
    match bone {
        Bone::LeftThumbMetacarpal => "leftThumbProximal",
        Bone::LeftThumbProximal => "leftThumbIntermediate",
        Bone::RightThumbMetacarpal => "rightThumbProximal",
        Bone::RightThumbProximal => "rightThumbIntermediate",
        bone => bone.name(),
    }
}

pub(crate) const UNKNOWN_PRESET: &str = "unknown";

pub(crate) fn preset_names(preset: ExpressionPreset) -> Option<(&'static str, &'static str)> {
    Some(match preset {
        ExpressionPreset::Neutral => ("neutral", "Neutral"),
        ExpressionPreset::Aa => ("a", "A"),
        ExpressionPreset::Ih => ("i", "I"),
        ExpressionPreset::Ou => ("u", "U"),
        ExpressionPreset::Ee => ("e", "E"),
        ExpressionPreset::Oh => ("o", "O"),
        ExpressionPreset::Blink => ("blink", "Blink"),
        ExpressionPreset::BlinkLeft => ("blink_l", "Blink_L"),
        ExpressionPreset::BlinkRight => ("blink_r", "Blink_R"),
        ExpressionPreset::Happy => ("joy", "Joy"),
        ExpressionPreset::Angry => ("angry", "Angry"),
        ExpressionPreset::Sad => ("sorrow", "Sorrow"),
        ExpressionPreset::Relaxed => ("fun", "Fun"),
        ExpressionPreset::LookUp => ("lookup", "LookUp"),
        ExpressionPreset::LookDown => ("lookdown", "LookDown"),
        ExpressionPreset::LookLeft => ("lookleft", "LookLeft"),
        ExpressionPreset::LookRight => ("lookright", "LookRight"),
        ExpressionPreset::Surprised => return None,
    })
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Vrm {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exporter_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub humanoid: Option<Humanoid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_person: Option<FirstPerson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend_shape_master: Option<BlendShapeMaster>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_animation: Option<SecondaryAnimation>,
    #[serde(default)]
    pub material_properties: Vec<MaterialProperties>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Meta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_information: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_user_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub violent_ussage_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sexual_ussage_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commercial_ussage_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other_permission_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other_license_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Humanoid {
    #[serde(default)]
    pub human_bones: Vec<HumanBone>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arm_stretch: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leg_stretch: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_arm_twist: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_arm_twist: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_leg_twist: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_leg_twist: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feet_spacing: Option<f32>,
    #[serde(
        rename = "hasTranslationDoF",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub has_translation_dof: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HumanBone {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_default_values: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<Vector3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<Vector3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<Vector3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis_length: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FirstPerson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_person_bone: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_person_bone_offset: Option<Vector3>,
    #[serde(default)]
    pub mesh_annotations: Vec<MeshAnnotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_at_type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_at_horizontal_inner: Option<DegreeMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_at_horizontal_outer: Option<DegreeMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_at_vertical_down: Option<DegreeMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_at_vertical_up: Option<DegreeMap>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub(crate) struct Vector3 {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
}

impl Vector3 {
    pub(crate) fn to_gltf(self) -> Vec3 {
        Vec3::new(self.x, self.y, -self.z)
    }

    pub(crate) fn from_gltf(vector: Vec3) -> Self {
        Self {
            x: vector.x,
            y: vector.y,
            z: -vector.z,
        }
    }

    pub(crate) fn to_vec3(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    pub(crate) fn from_vec3(vector: Vec3) -> Self {
        Self {
            x: vector.x,
            y: vector.y,
            z: vector.z,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeshAnnotation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_person_flag: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DegreeMap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_range: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_range: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlendShapeMaster {
    #[serde(default)]
    pub blend_shape_groups: Vec<BlendShapeGroup>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlendShapeGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_name: Option<String>,
    #[serde(default)]
    pub binds: Vec<BlendShapeBind>,
    #[serde(default)]
    pub material_values: Vec<MaterialValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_binary: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlendShapeBind {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MaterialValue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property_name: Option<String>,
    #[serde(default)]
    pub target_value: Vec<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecondaryAnimation {
    #[serde(default)]
    pub bone_groups: Vec<BoneGroup>,
    #[serde(default)]
    pub collider_groups: Vec<ColliderGroup>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BoneGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stiffiness: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity_power: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity_dir: Option<Vector3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_force: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_radius: Option<f32>,
    #[serde(default)]
    pub bones: Vec<Index>,
    #[serde(default)]
    pub collider_groups: Vec<Index>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ColliderGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<Index>,
    #[serde(default)]
    pub colliders: Vec<Collider>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Collider {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<Vector3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MaterialProperties {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shader: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_queue: Option<i32>,
    #[serde(default)]
    pub float_properties: BTreeMap<String, f32>,
    #[serde(default)]
    pub vector_properties: BTreeMap<String, Vec<f32>>,
    #[serde(default)]
    pub texture_properties: BTreeMap<String, Index>,
    #[serde(default)]
    pub keyword_map: BTreeMap<String, Value>,
    #[serde(default)]
    pub tag_map: BTreeMap<String, Value>,
}

impl MaterialProperties {
    pub(crate) fn float(&self, name: &str) -> Option<f32> {
        self.float_properties.get(name).copied()
    }

    pub(crate) fn keyword(&self, name: &str) -> bool {
        self.keyword_map
            .get(name)
            .is_some_and(|value| value.as_bool().unwrap_or(true))
    }

    pub(crate) fn texture(&self, name: &str) -> Option<Index> {
        self.texture_properties.get(name).copied()
    }

    pub(crate) fn color(&self, name: &str) -> Option<[f32; 4]> {
        let color = self.vector(name)?;
        Some([
            linear(color[0]),
            linear(color[1]),
            linear(color[2]),
            color[3],
        ])
    }

    pub(crate) fn vector(&self, name: &str) -> Option<[f32; 4]> {
        let values = self.vector_properties.get(name)?;
        let mut vector = [1.0; 4];
        for (slot, value) in vector.iter_mut().zip(values) {
            *slot = *value;
        }
        Some(vector)
    }

    pub(crate) fn texture_transform(&self, name: &str) -> Option<([f32; 2], [f32; 2])> {
        let values = self.vector_properties.get(name)?;
        if values.len() < 4 {
            return None;
        }
        Some(([values[0], values[1]], [values[2], values[3]]))
    }
}

pub(crate) fn linear(value: f32) -> f32 {
    value.max(0.0).powf(2.2)
}

pub(crate) fn gamma(value: f32) -> f32 {
    value.max(0.0).powf(1.0 / 2.2)
}
