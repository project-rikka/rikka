use rikka_model_core::glam::Vec4;

use rikka_model_core::ExportCtx;
use rikka_model_core::Warning;
use rikka_model_core::{HumanBone, Model, NodeId};

use super::super::super::avatar::Vrm;
use super::super::super::expression::{
    ExpressionName, ExpressionOverride, MaterialColor, MaterialColorBind, TextureTransformBind,
};
use super::super::super::legacy as raw;
use super::super::super::legacy_humanoid::{BoneLimit, LegacyHumanoid};
use super::super::super::license::LegacyLicense;
use super::super::super::look_at::{LookAtKind, RangeMap};
use super::super::super::look_at_curves::LookAtCurves;
use super::super::super::meta::Meta;
use super::super::super::vrm_legacy::VrmLegacy;

const LEGACY_REQUIRED: &[HumanBone] = &[HumanBone::Chest, HumanBone::Neck];

pub(super) struct Sources<'a> {
    pub model: &'a Model,
    pub vrm: &'a Vrm,
    pub legacy: Option<&'a VrmLegacy>,
    pub meshes: &'a [Option<u32>],
}

impl Sources<'_> {
    fn mesh_of(&self, node: NodeId) -> Option<u32> {
        let mesh = self.model.nodes.get(node.index())?.mesh?;
        self.meshes.get(mesh.index()).copied().flatten()
    }
}

pub(super) fn meta(
    meta: &Meta,
    legacy: Option<&VrmLegacy>,
    thumbnail: Option<u32>,
    ctx: &mut ExportCtx<'_>,
) -> raw::Meta {
    let license = legacy.map_or_else(
        || LegacyLicense::from_meta(meta),
        |legacy| legacy.license.clone(),
    );
    let losses = [
        (meta.copyright.is_some(), "copyright"),
        (meta.third_party_licenses.is_some(), "third-party licenses"),
        (
            meta.allow_political_or_religious_usage,
            "political or religious usage permission",
        ),
        (
            meta.allow_antisocial_or_hate_usage,
            "antisocial or hate usage permission",
        ),
        (meta.authors.len() > 1, "separate authors"),
        (meta.references.len() > 1, "separate references"),
    ];
    let names: Vec<&str> = losses
        .into_iter()
        .filter(|(lost, _)| *lost)
        .map(|(_, name)| name)
        .collect();
    if !names.is_empty() {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 0.x meta cannot store the {}",
            names.join(", ")
        )));
    }
    let usage = |allowed: bool| {
        Some(
            match allowed {
                true => "Allow",
                false => "Disallow",
            }
            .to_owned(),
        )
    };
    let joined = |values: &[String]| Some(values.join(", ")).filter(|value| !value.is_empty());
    raw::Meta {
        title: Some(meta.name.clone()),
        version: meta.version.clone(),
        author: joined(&meta.authors),
        contact_information: meta.contact.clone(),
        reference: joined(&meta.references),
        texture: thumbnail.map(|texture| texture as i32),
        allowed_user_name: Some(license.allowed_user.name().to_owned()),
        violent_ussage_name: usage(license.violent_usage),
        sexual_ussage_name: usage(license.sexual_usage),
        commercial_ussage_name: usage(license.commercial_usage),
        other_permission_url: license.other_permission_url,
        license_name: Some(license.license.name().to_owned()),
        other_license_url: license.other_license_url,
    }
}

pub(super) fn humanoid(sources: &Sources<'_>, ctx: &mut ExportCtx<'_>) -> raw::Humanoid {
    let humanoid = &sources.vrm.humanoid;
    let missing: Vec<&str> = humanoid
        .missing()
        .chain(
            LEGACY_REQUIRED
                .iter()
                .copied()
                .filter(|bone| humanoid.bone(*bone).is_none()),
        )
        .map(raw::bone_name)
        .collect();
    if !missing.is_empty() {
        ctx.warn(Warning::Repaired(format!(
            "VRM 0.x humanoid is missing required bone {}",
            missing.join(", ")
        )));
    }
    let description = sources
        .legacy
        .map_or_else(LegacyHumanoid::default, |legacy| legacy.humanoid);
    raw::Humanoid {
        human_bones: humanoid
            .bones()
            .map(|(bone, node)| {
                let limit = sources
                    .legacy
                    .and_then(|legacy| legacy.bone_limits.get(&bone).copied());
                human_bone(bone, node, limit)
            })
            .collect(),
        arm_stretch: Some(description.arm_stretch),
        leg_stretch: Some(description.leg_stretch),
        upper_arm_twist: Some(description.upper_arm_twist),
        lower_arm_twist: Some(description.lower_arm_twist),
        upper_leg_twist: Some(description.upper_leg_twist),
        lower_leg_twist: Some(description.lower_leg_twist),
        feet_spacing: Some(description.feet_spacing),
        has_translation_dof: Some(description.has_translation_dof),
    }
}

fn human_bone(bone: HumanBone, node: NodeId, limit: Option<BoneLimit>) -> raw::HumanBone {
    let described = raw::HumanBone {
        bone: Some(raw::bone_name(bone).to_owned()),
        node: Some(node.0),
        use_default_values: Some(true),
        ..raw::HumanBone::default()
    };
    match limit {
        None => described,
        Some(limit) => raw::HumanBone {
            use_default_values: Some(limit.use_default_values),
            min: Some(raw::Vector3::from_vec3(limit.min)),
            max: Some(raw::Vector3::from_vec3(limit.max)),
            center: Some(raw::Vector3::from_vec3(limit.center)),
            axis_length: Some(limit.axis_length),
            ..described
        },
    }
}

pub(super) fn first_person(sources: &Sources<'_>, ctx: &mut ExportCtx<'_>) -> raw::FirstPerson {
    let vrm = sources.vrm;
    let look_at = &vrm.look_at;
    let curves = sources.legacy.map_or_else(LookAtCurves::default, |legacy| {
        legacy.look_at_curves.clone()
    });
    let bone = sources
        .legacy
        .and_then(|legacy| legacy.first_person_bone)
        .or_else(|| vrm.humanoid.bone(HumanBone::Head));

    let mut annotations: Vec<raw::MeshAnnotation> = Vec::new();
    let mut unplaced = 0;
    let mut conflicting = 0;
    for annotation in &vrm.first_person {
        let Some(mesh) = sources.mesh_of(annotation.node) else {
            unplaced += 1;
            continue;
        };
        let flag = capitalized(annotation.kind.name());
        let mesh = mesh as i32;
        match annotations
            .iter()
            .find(|written| written.mesh == Some(mesh))
        {
            Some(written) if written.first_person_flag.as_ref() != Some(&flag) => {
                conflicting += 1;
            }
            Some(_) => {}
            None => annotations.push(raw::MeshAnnotation {
                mesh: Some(mesh),
                first_person_flag: Some(flag),
            }),
        }
    }
    if unplaced > 0 {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 0.x annotates meshes, dropping {unplaced} first-person annotations of nodes without a mesh"
        )));
    }
    if conflicting > 0 {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 0.x annotates meshes, {conflicting} nodes sharing a mesh keep its first annotation"
        )));
    }

    let degree_map = |range: RangeMap, curve: &[f32]| raw::DegreeMap {
        curve: Some(curve.to_vec()),
        x_range: Some(range.input_max_degrees),
        y_range: Some(range.output_scale),
    };
    raw::FirstPerson {
        first_person_bone: bone.map(|node| node.0 as i32),
        first_person_bone_offset: Some(raw::Vector3::from_gltf(look_at.offset_from_head)),
        mesh_annotations: annotations,
        look_at_type_name: Some(
            match look_at.kind {
                LookAtKind::Bone => "Bone",
                LookAtKind::Expression => "BlendShape",
            }
            .to_owned(),
        ),
        look_at_horizontal_inner: Some(degree_map(
            look_at.horizontal_inner,
            &curves.horizontal_inner,
        )),
        look_at_horizontal_outer: Some(degree_map(
            look_at.horizontal_outer,
            &curves.horizontal_outer,
        )),
        look_at_vertical_down: Some(degree_map(look_at.vertical_down, &curves.vertical_down)),
        look_at_vertical_up: Some(degree_map(look_at.vertical_up, &curves.vertical_up)),
    }
}

fn capitalized(name: &str) -> String {
    let mut characters = name.chars();
    match characters.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + characters.as_str(),
        None => String::new(),
    }
}

pub(super) fn blend_shape_master(
    sources: &Sources<'_>,
    ctx: &mut ExportCtx<'_>,
) -> raw::BlendShapeMaster {
    let mut overridden = Vec::new();
    let mut unbound = 0;
    let mut matcap = 0;
    let groups = sources
        .vrm
        .expressions
        .iter()
        .map(|expression| {
            let (preset_name, default_name) = match &expression.name {
                ExpressionName::Preset(preset) => raw::preset_names(*preset)
                    .map(|(preset_name, name)| (preset_name, name.to_owned()))
                    .unwrap_or_else(|| (raw::UNKNOWN_PRESET, capitalized(preset.name()))),
                ExpressionName::Custom(name) => (raw::UNKNOWN_PRESET, name.clone()),
            };
            let legacy = sources
                .legacy
                .and_then(|legacy| legacy.expression(&expression.name));
            let name = legacy
                .map(|legacy| legacy.group_name.clone())
                .filter(|name| !name.is_empty())
                .unwrap_or(default_name);
            if [
                expression.override_blink,
                expression.override_look_at,
                expression.override_mouth,
            ]
            .iter()
            .any(|mode| *mode != ExpressionOverride::None)
            {
                overridden.push(name.clone());
            }

            let mut binds: Vec<raw::BlendShapeBind> = Vec::new();
            for bind in &expression.morph_binds {
                let Some(mesh) = sources.mesh_of(bind.node) else {
                    unbound += 1;
                    continue;
                };
                let mesh = mesh as i32;
                if binds
                    .iter()
                    .any(|written| written.mesh == Some(mesh) && written.index == Some(bind.index))
                {
                    continue;
                }
                binds.push(raw::BlendShapeBind {
                    mesh: Some(mesh),
                    index: Some(bind.index),
                    weight: Some((bind.weight * 100.0).clamp(0.0, 100.0)),
                });
            }

            let mut material_values: Vec<raw::MaterialValue> = Vec::new();
            for bind in &expression.material_color_binds {
                match color_value(sources.model, bind) {
                    Some(value) => material_values.push(value),
                    None => matcap += 1,
                }
            }
            for bind in &expression.texture_transform_binds {
                material_values.push(texture_value(sources.model, bind));
            }
            for value in legacy
                .into_iter()
                .flat_map(|legacy| &legacy.material_values)
            {
                let Some(material) = sources.model.materials.get(value.material.index()) else {
                    continue;
                };
                material_values.push(raw::MaterialValue {
                    material_name: Some(material.name.clone()),
                    property_name: Some(value.property.clone()),
                    target_value: value.target_value.clone(),
                });
            }

            raw::BlendShapeGroup {
                name: Some(name),
                preset_name: Some(preset_name.to_owned()),
                binds,
                material_values,
                is_binary: Some(expression.is_binary),
            }
        })
        .collect();
    if !overridden.is_empty() {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 0.x has no expression overrides, dropping those of {}",
            overridden
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if unbound > 0 {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 0.x binds morph targets by mesh, dropping {unbound} binds of nodes without a mesh"
        )));
    }
    if matcap > 0 {
        ctx.warn(Warning::Unsupported(format!(
            "MToon 0.x has no matcap colour, dropping {matcap} expression binds"
        )));
    }
    raw::BlendShapeMaster {
        blend_shape_groups: groups,
    }
}

fn color_value(model: &Model, bind: &MaterialColorBind) -> Option<raw::MaterialValue> {
    let property = match bind.color {
        MaterialColor::Base => "_Color",
        MaterialColor::Emissive => "_EmissionColor",
        MaterialColor::Shade => "_ShadeColor",
        MaterialColor::Rim => "_RimColor",
        MaterialColor::Outline => "_OutlineColor",
        MaterialColor::Matcap => return None,
    };
    Some(raw::MaterialValue {
        material_name: Some(model.material(bind.material).name.clone()),
        property_name: Some(property.to_owned()),
        target_value: bind.value.to_array().to_vec(),
    })
}

fn texture_value(model: &Model, bind: &TextureTransformBind) -> raw::MaterialValue {
    let target = Vec4::new(
        bind.scale.x,
        bind.scale.y,
        bind.offset.x,
        1.0 - bind.scale.y - bind.offset.y,
    );
    raw::MaterialValue {
        material_name: Some(model.material(bind.material).name.clone()),
        property_name: Some("_MainTex_ST".to_owned()),
        target_value: target.to_array().to_vec(),
    }
}
