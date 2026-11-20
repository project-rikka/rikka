use std::collections::{BTreeMap, BTreeSet};

use rikka_model_core::glam::{Vec2, Vec3, Vec4};

use rikka_gltf::SourceDocument;
use rikka_gltf::TextureMap;
use rikka_model_core::HumanBone;
use rikka_model_core::ImportCtx;
use rikka_model_core::Model;
use rikka_model_core::Warning;
use rikka_model_core::{Alpha, Material, Outline, OutlineWidth, Shading, Toon, ToonSurface};

use super::super::avatar::{Vrm, VrmVersion};
use super::super::expression::{
    Expression, ExpressionName, ExpressionPreset, MaterialColor, MaterialColorBind, MorphBind,
    TextureTransformBind,
};
use super::super::first_person::{FirstPersonKind, MeshAnnotation};
use super::super::legacy as raw;
use super::super::legacy_expression::{LegacyExpression, LegacyMaterialValue};
use super::super::legacy_humanoid::{BoneLimit, LegacyHumanoid};
use super::super::license::{LegacyLicense, LegacyUser, LicenseName};
use super::super::look_at::{LookAt, LookAtKind, RangeMap};
use super::super::look_at_curves::LookAtCurves;
use super::super::meta::Meta;
use super::super::spring::{
    Collider, ColliderGroup, ColliderShape, Spring, SpringBone, SpringJoint,
};
use super::super::vrm_legacy::VrmLegacy;
use super::mtoon;
use super::resolve::Resolved;

pub(super) fn described(document: &SourceDocument, ctx: &mut ImportCtx<'_>) -> Option<raw::Vrm> {
    super::read_extension(document.extension(raw::VRM), raw::VRM, ctx)
}

pub(super) fn read(
    described: &raw::Vrm,
    model: &Model,
    texture_map: &TextureMap,
    meshes: &[Option<rikka_model_core::MeshId>],
    ctx: &mut ImportCtx<'_>,
) -> (Vrm, VrmLegacy) {
    let resolved = Resolved::new(model, meshes);
    let first_person_settings = described.first_person.as_ref();
    let license = license(described.meta.as_ref());
    let (expressions, legacy_expressions) = expressions(described, &resolved, ctx);
    let vrm = Vrm {
        version: VrmVersion::Legacy,
        meta: meta(described.meta.as_ref(), &license, texture_map),
        humanoid: super::humanoid(
            described
                .humanoid
                .iter()
                .flat_map(|humanoid| humanoid.human_bones.iter())
                .filter_map(|bone| Some((bone.bone.as_deref()?, bone.node?))),
            true,
            &resolved,
            ctx,
        ),
        first_person: first_person(first_person_settings, &resolved),
        look_at: look_at(first_person_settings),
        expressions,
        spring_bone: spring_bone(described, &resolved, ctx),
        constraints: Vec::new(),
    };
    let legacy = VrmLegacy {
        license,
        humanoid: legacy_humanoid(described.humanoid.as_ref()),
        bone_limits: bone_limits(described.humanoid.as_ref()),
        first_person_bone: first_person_settings
            .and_then(|settings| settings.first_person_bone)
            .and_then(|node| u32::try_from(node).ok())
            .and_then(|node| resolved.node(node, "first-person bone", ctx)),
        look_at_curves: look_at_curves(first_person_settings),
        expressions: legacy_expressions,
    };
    (vrm, legacy)
}

fn license(described: Option<&raw::Meta>) -> LegacyLicense {
    let Some(described) = described else {
        return LegacyLicense::default();
    };
    let allowed = |name: &Option<String>| name.as_deref() == Some("Allow");
    LegacyLicense {
        allowed_user: described
            .allowed_user_name
            .as_deref()
            .and_then(LegacyUser::from_name)
            .unwrap_or_default(),
        violent_usage: allowed(&described.violent_ussage_name),
        sexual_usage: allowed(&described.sexual_ussage_name),
        commercial_usage: allowed(&described.commercial_ussage_name),
        other_permission_url: described.other_permission_url.clone(),
        license: described
            .license_name
            .as_deref()
            .and_then(LicenseName::from_name)
            .unwrap_or(LicenseName::Other),
        other_license_url: described.other_license_url.clone(),
    }
}

fn meta(described: Option<&raw::Meta>, license: &LegacyLicense, texture_map: &TextureMap) -> Meta {
    let mut meta = Meta {
        license_url: "https://vrm.dev/licenses/1.0/".to_owned(),
        ..Meta::default()
    };
    license.apply_to(&mut meta);
    let Some(described) = described else {
        return meta;
    };
    meta.name = described.title.clone().unwrap_or_default();
    meta.version = described.version.clone();
    meta.authors = described.author.clone().into_iter().collect();
    meta.contact = described.contact_information.clone();
    meta.references = described.reference.clone().into_iter().collect();
    meta.thumbnail = described
        .texture
        .and_then(|texture| u32::try_from(texture).ok())
        .and_then(|texture| texture_map.image(texture));
    meta
}

fn legacy_humanoid(described: Option<&raw::Humanoid>) -> LegacyHumanoid {
    let defaults = LegacyHumanoid::default();
    let Some(described) = described else {
        return defaults;
    };
    LegacyHumanoid {
        arm_stretch: described.arm_stretch.unwrap_or(defaults.arm_stretch),
        leg_stretch: described.leg_stretch.unwrap_or(defaults.leg_stretch),
        upper_arm_twist: described
            .upper_arm_twist
            .unwrap_or(defaults.upper_arm_twist),
        lower_arm_twist: described
            .lower_arm_twist
            .unwrap_or(defaults.lower_arm_twist),
        upper_leg_twist: described
            .upper_leg_twist
            .unwrap_or(defaults.upper_leg_twist),
        lower_leg_twist: described
            .lower_leg_twist
            .unwrap_or(defaults.lower_leg_twist),
        feet_spacing: described.feet_spacing.unwrap_or(defaults.feet_spacing),
        has_translation_dof: described
            .has_translation_dof
            .unwrap_or(defaults.has_translation_dof),
    }
}

fn bone_limits(described: Option<&raw::Humanoid>) -> BTreeMap<HumanBone, BoneLimit> {
    let Some(described) = described else {
        return BTreeMap::new();
    };
    described
        .human_bones
        .iter()
        .filter_map(|bone| {
            let human_bone = raw::bone_from_name(bone.bone.as_deref()?)?;
            let defaults = BoneLimit::default();
            let limit = BoneLimit {
                use_default_values: bone
                    .use_default_values
                    .unwrap_or(defaults.use_default_values),
                min: bone.min.map_or(defaults.min, raw::Vector3::to_vec3),
                max: bone.max.map_or(defaults.max, raw::Vector3::to_vec3),
                center: bone.center.map_or(defaults.center, raw::Vector3::to_vec3),
                axis_length: bone.axis_length.unwrap_or(defaults.axis_length),
            };
            (limit != defaults).then_some((human_bone, limit))
        })
        .collect()
}

fn first_person(
    described: Option<&raw::FirstPerson>,
    resolved: &Resolved<'_>,
) -> Vec<MeshAnnotation> {
    let Some(described) = described else {
        return Vec::new();
    };
    described
        .mesh_annotations
        .iter()
        .filter_map(|annotation| {
            let mesh = resolved.mesh(u32::try_from(annotation.mesh?).ok()?)?;
            let kind = annotation
                .first_person_flag
                .as_deref()
                .and_then(FirstPersonKind::from_name)
                .unwrap_or_default();
            Some(
                resolved
                    .nodes_of(mesh)
                    .map(move |node| MeshAnnotation { node, kind })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

fn look_at(described: Option<&raw::FirstPerson>) -> LookAt {
    let Some(described) = described else {
        return LookAt::default();
    };
    let kind = described
        .look_at_type_name
        .as_deref()
        .and_then(LookAtKind::from_name)
        .unwrap_or_default();
    let map = |degree_map: Option<&raw::DegreeMap>| match degree_map {
        Some(degree_map) => RangeMap::new(
            degree_map.x_range.unwrap_or(90.0),
            degree_map.y_range.unwrap_or(kind.default_output_scale()),
        ),
        None => RangeMap::new(90.0, kind.default_output_scale()),
    };
    LookAt {
        offset_from_head: described
            .first_person_bone_offset
            .map(raw::Vector3::to_gltf)
            .unwrap_or(Vec3::new(0.0, 0.06, 0.0)),
        kind,
        horizontal_inner: map(described.look_at_horizontal_inner.as_ref()),
        horizontal_outer: map(described.look_at_horizontal_outer.as_ref()),
        vertical_down: map(described.look_at_vertical_down.as_ref()),
        vertical_up: map(described.look_at_vertical_up.as_ref()),
    }
}

fn look_at_curves(described: Option<&raw::FirstPerson>) -> LookAtCurves {
    let defaults = LookAtCurves::default();
    let Some(described) = described else {
        return defaults;
    };
    let curve = |degree_map: Option<&raw::DegreeMap>, fallback: Vec<f32>| {
        degree_map
            .and_then(|degree_map| degree_map.curve.clone())
            .unwrap_or(fallback)
    };
    LookAtCurves {
        horizontal_inner: curve(
            described.look_at_horizontal_inner.as_ref(),
            defaults.horizontal_inner,
        ),
        horizontal_outer: curve(
            described.look_at_horizontal_outer.as_ref(),
            defaults.horizontal_outer,
        ),
        vertical_down: curve(
            described.look_at_vertical_down.as_ref(),
            defaults.vertical_down,
        ),
        vertical_up: curve(described.look_at_vertical_up.as_ref(), defaults.vertical_up),
    }
}

fn expressions(
    described: &raw::Vrm,
    resolved: &Resolved<'_>,
    ctx: &mut ImportCtx<'_>,
) -> (Vec<Expression>, Vec<LegacyExpression>) {
    let Some(master) = &described.blend_shape_master else {
        return (Vec::new(), Vec::new());
    };
    master
        .blend_shape_groups
        .iter()
        .map(|group| {
            let name = expression_name(group);
            let what = format!("expression \"{name}\"");
            let mut expression = Expression::new(name.clone());
            let mut legacy = LegacyExpression::new(name, group.name.clone().unwrap_or_default());
            expression.is_binary = group.is_binary.unwrap_or(false);
            for bind in &group.binds {
                let Some(mesh) = bind
                    .mesh
                    .and_then(|mesh| u32::try_from(mesh).ok())
                    .and_then(|mesh| resolved.mesh(mesh))
                else {
                    continue;
                };
                let index = bind.index.unwrap_or(0);
                let weight = bind.weight.unwrap_or(100.0) / 100.0;
                for node in resolved.nodes_of(mesh).collect::<Vec<_>>() {
                    if resolved.morph(node, index, &what, ctx).is_some() {
                        expression.morph_binds.push(MorphBind {
                            node,
                            index,
                            weight,
                        });
                    }
                }
            }
            for value in &group.material_values {
                material_bind(&mut expression, &mut legacy, value, resolved, ctx);
            }
            (expression, legacy)
        })
        .unzip()
}

fn expression_name(group: &raw::BlendShapeGroup) -> ExpressionName {
    let name = group.name.clone().unwrap_or_default();
    let preset = match group.preset_name.as_deref() {
        Some(preset) if preset != raw::UNKNOWN_PRESET => ExpressionPreset::from_legacy_name(preset),
        _ => ExpressionPreset::from_legacy_name(&name),
    };
    match preset {
        Some(preset) => ExpressionName::Preset(preset),
        None => ExpressionName::Custom(name),
    }
}

fn material_bind(
    expression: &mut Expression,
    legacy: &mut LegacyExpression,
    value: &raw::MaterialValue,
    resolved: &Resolved<'_>,
    ctx: &mut ImportCtx<'_>,
) {
    let Some(name) = value.material_name.as_deref() else {
        return;
    };
    let Some(material) = resolved.material_named(name) else {
        ctx.warn(Warning::Repaired(format!(
            "expression references missing material \"{name}\""
        )));
        return;
    };
    let property = value.property_name.as_deref().unwrap_or_default();
    let mut target = [0.0; 4];
    for (slot, value) in target.iter_mut().zip(&value.target_value) {
        *slot = *value;
    }

    if property == "_MainTex_ST" && value.target_value.len() >= 4 {
        expression
            .texture_transform_binds
            .push(TextureTransformBind {
                material,
                scale: Vec2::new(target[0], target[1]),
                offset: Vec2::new(target[2], 1.0 - target[1] - target[3]),
            });
        return;
    }
    let color = match property {
        "_Color" => MaterialColor::Base,
        "_EmissionColor" => MaterialColor::Emissive,
        "_ShadeColor" => MaterialColor::Shade,
        "_RimColor" => MaterialColor::Rim,
        "_OutlineColor" => MaterialColor::Outline,
        _ => {
            legacy.material_values.push(LegacyMaterialValue {
                material,
                property: property.to_owned(),
                target_value: value.target_value.clone(),
            });
            return;
        }
    };
    expression.material_color_binds.push(MaterialColorBind {
        material,
        color,
        value: Vec4::from_array(target),
    });
}

fn spring_bone(
    described: &raw::Vrm,
    resolved: &Resolved<'_>,
    ctx: &mut ImportCtx<'_>,
) -> SpringBone {
    let Some(secondary) = &described.secondary_animation else {
        return SpringBone::default();
    };
    let mut bone = SpringBone::default();

    for group in &secondary.collider_groups {
        let node = group
            .node
            .and_then(|node| resolved.node(node, "spring bone collider group", ctx));
        let mut colliders = Vec::new();
        if let Some(node) = node {
            for collider in &group.colliders {
                colliders.push(bone.colliders.len());
                bone.colliders.push(Collider {
                    node,
                    shape: ColliderShape::Sphere {
                        offset: collider
                            .offset
                            .map(raw::Vector3::to_gltf)
                            .unwrap_or(Vec3::ZERO),
                        radius: collider.radius.unwrap_or(0.0),
                    },
                });
            }
        }
        bone.collider_groups.push(ColliderGroup {
            name: String::new(),
            colliders,
        });
    }

    for group in &secondary.bone_groups {
        let joints = |root: rikka_model_core::NodeId| {
            let mut joints = Vec::new();
            let mut cursor = Some(root);
            while let Some(node) = cursor {
                joints.push(SpringJoint {
                    node,
                    hit_radius: group.hit_radius.unwrap_or(0.0),
                    stiffness: group.stiffiness.unwrap_or(1.0),
                    gravity_power: group.gravity_power.unwrap_or(0.0),
                    gravity_direction: group
                        .gravity_dir
                        .map(raw::Vector3::to_gltf)
                        .unwrap_or(Vec3::NEG_Y),
                    drag: group.drag_force.unwrap_or(0.5),
                });
                cursor = resolved.model().node(node).children.first().copied();
            }
            joints
        };

        let collider_groups: Vec<usize> = group
            .collider_groups
            .iter()
            .map(|index| *index as usize)
            .filter(|index| *index < bone.collider_groups.len())
            .collect();
        let center = group
            .center
            .filter(|center| *center >= 0)
            .and_then(|center| resolved.node(center as u32, "spring bone centre", ctx));

        for root in &group.bones {
            let Some(root) = resolved.node(*root, "spring bone chain", ctx) else {
                continue;
            };
            bone.springs.push(Spring {
                name: group.comment.clone().unwrap_or_default(),
                joints: joints(root),
                collider_groups: collider_groups.clone(),
                center,
            });
        }
    }
    bone
}

pub(super) fn materials(
    described: &raw::Vrm,
    model: &mut Model,
    texture_map: &TextureMap,
    ctx: &mut ImportCtx<'_>,
) {
    let queues = RenderQueues::from_raw(described);
    let mut unknown = BTreeSet::new();
    for (index, properties) in described.material_properties.iter().enumerate() {
        let Some(material) = model.materials.get_mut(index) else {
            continue;
        };
        match properties.shader.as_deref() {
            Some("VRM/MToon") => toon(material, properties, texture_map, &queues),
            Some(shader) if shader.starts_with("VRM/Unlit") => {
                unlit(material, properties, texture_map, &queues);
            }
            Some("VRM_USE_GLTFSHADER") | None => {}
            Some(shader) => {
                unknown.insert(shader.to_owned());
            }
        }
    }
    for shader in unknown {
        ctx.warn(Warning::Unsupported(format!(
            "ignoring unsupported shader \"{shader}\""
        )));
    }
}

struct RenderQueues {
    transparent: Vec<i32>,
    with_depth: Vec<i32>,
}

impl RenderQueues {
    fn from_raw(described: &raw::Vrm) -> Self {
        let mut transparent = BTreeSet::new();
        let mut with_depth = BTreeSet::new();
        for properties in &described.material_properties {
            let shader = properties.shader.as_deref().unwrap_or_default();
            let depth_written =
                properties.float("_ZWrite") == Some(1.0) || shader == "VRM/UnlitTransparentZWrite";
            let blended = properties.keyword("_ALPHABLEND_ON")
                || shader == "VRM/UnlitTransparent"
                || shader == "VRM/UnlitTransparentZWrite";
            let Some(queue) = properties.render_queue.filter(|_| blended) else {
                continue;
            };
            match depth_written {
                true => with_depth.insert(queue),
                false => transparent.insert(queue),
            };
        }
        Self {
            transparent: transparent.into_iter().collect(),
            with_depth: with_depth.into_iter().collect(),
        }
    }

    fn offset(&self, queue: Option<i32>, depth_written: bool) -> i32 {
        let Some(queue) = queue else {
            return 0;
        };
        match depth_written {
            true => self
                .with_depth
                .iter()
                .position(|candidate| *candidate == queue)
                .map_or(0, |place| (place as i32).clamp(0, 9)),
            false => self
                .transparent
                .iter()
                .position(|candidate| *candidate == queue)
                .map_or(0, |place| {
                    (place as i32 - self.transparent.len() as i32 + 1).clamp(-9, 0)
                }),
        }
    }
}

fn toon(
    material: &mut Material,
    properties: &raw::MaterialProperties,
    texture_map: &TextureMap,
    queues: &RenderQueues,
) {
    let blended = properties.keyword("_ALPHABLEND_ON");
    let depth_written = properties.float("_ZWrite") == Some(1.0);
    surface(material, properties, texture_map, blended);

    let shift = properties.float("_ShadeShift").unwrap_or(0.0);
    let toony = properties.float("_ShadeToony").unwrap_or(0.9);
    let lowest = shift;
    let highest = 1.0 + (shift - 1.0) * toony;
    let shading_toony = ((2.0 - (highest - lowest)) * 0.5).clamp(0.0, 1.0);
    let shading_shift = (-(highest + lowest) * 0.5).clamp(-1.0, 1.0);
    let indirect = properties.float("_IndirectLightIntensity").unwrap_or(0.1);
    let outline_mode = properties.float("_OutlineWidthMode").unwrap_or(0.0) as i32;
    let outline_width = properties.float("_OutlineWidth").unwrap_or(0.0);
    let lighting_mix = match properties.float("_OutlineColorMode").unwrap_or(0.0) as i32 {
        1 => properties.float("_OutlineLightingMix").unwrap_or(1.0),
        _ => 0.0,
    };

    let textures = &mut material.textures;
    textures.shade = properties
        .texture("_ShadeTexture")
        .and_then(|index| texture_map.texture(index));
    textures.matcap = properties
        .texture("_SphereAdd")
        .and_then(|index| texture_map.texture(index));
    textures.rim = properties
        .texture("_RimTexture")
        .and_then(|index| texture_map.texture(index));
    textures.outline_width = properties
        .texture("_OutlineWidthTexture")
        .and_then(|index| texture_map.texture(index));
    textures.uv_animation_mask = properties
        .texture("_UvAnimMaskTexture")
        .and_then(|index| texture_map.texture(index));

    material.shading = Shading::Toon(Toon {
        shade_color: color(properties, "_ShadeColor", Vec3::new(0.97, 0.81, 0.86)),
        shading_shift,
        shading_shift_scale: 1.0,
        shading_toony,
        gi_equalization: (1.0 - indirect).clamp(0.0, 1.0),
        normal_scale: properties.float("_BumpScale").unwrap_or(1.0),
        matcap_color: Vec3::ONE,
        rim_color: color(properties, "_RimColor", Vec3::ZERO),
        rim_fresnel_power: properties.float("_RimFresnelPower").unwrap_or(1.0),
        rim_lift: properties.float("_RimLift").unwrap_or(0.0),
        rim_lighting_mix: properties.float("_RimLightingMix").unwrap_or(0.0),
        outline: Outline {
            width: match outline_mode {
                1 => OutlineWidth::World(outline_width * raw::OUTLINE_WORLD_SCALE),
                2 => OutlineWidth::Screen(outline_width * raw::OUTLINE_SCREEN_SCALE),
                _ => OutlineWidth::None,
            },
            color: color(properties, "_OutlineColor", Vec3::ZERO),
            lighting_mix,
        },
        scroll: Vec2::new(
            properties.float("_UvAnimScrollX").unwrap_or(0.0),
            -properties.float("_UvAnimScrollY").unwrap_or(0.0),
        ),
        spin: properties.float("_UvAnimRotation").unwrap_or(0.0) * std::f32::consts::TAU,
        transparent_depth_write: depth_written && blended,
        render_order: queues.offset(properties.render_queue, depth_written && blended),
        surface: ToonSurface::classify(&material.name, None),
        ..Toon::default()
    });
}

fn unlit(
    material: &mut Material,
    properties: &raw::MaterialProperties,
    texture_map: &TextureMap,
    queues: &RenderQueues,
) {
    let shader = properties.shader.as_deref().unwrap_or_default();
    let depth_written = shader == "VRM/UnlitTransparentZWrite";
    let blended = depth_written || shader == "VRM/UnlitTransparent";
    surface(material, properties, texture_map, blended);
    let base = material.base_color.truncate();
    material.shading = Shading::Toon(Toon {
        shade_color: base,
        shading_toony: 1.0,
        normal_scale: 1.0,
        transparent_depth_write: depth_written,
        render_order: queues.offset(properties.render_queue, depth_written),
        surface: ToonSurface::classify(&material.name, None),
        ..Toon::default()
    });
    material.textures.shade = material.textures.base_color;
}

fn surface(
    material: &mut Material,
    properties: &raw::MaterialProperties,
    texture_map: &TextureMap,
    blended: bool,
) {
    let masked = properties.keyword("_ALPHATEST_ON");
    material.alpha = match (blended, masked) {
        (true, _) => Alpha::Blend,
        (false, true) => Alpha::Mask {
            cutoff: properties.float("_Cutoff").unwrap_or(0.5),
        },
        (false, false) => Alpha::Opaque,
    };
    material.double_sided = properties.float("_CullMode").unwrap_or(2.0) == 0.0;
    if let Some(color) = properties.color("_Color") {
        material.base_color = Vec4::from_array(color);
    }
    if let Some(color) = properties.vector("_EmissionColor") {
        material.emissive = Vec3::new(color[0], color[1], color[2]);
    }
    if let Some((offset, scale)) = properties.texture_transform("_MainTex") {
        material.uv_transform = mtoon::legacy_uv_transform(offset, scale);
    }

    let textures = &mut material.textures;
    textures.base_color = properties
        .texture("_MainTex")
        .and_then(|index| texture_map.texture(index));
    textures.normal = properties
        .texture("_BumpMap")
        .and_then(|index| texture_map.texture(index));
    textures.emissive = properties
        .texture("_EmissionMap")
        .and_then(|index| texture_map.texture(index));
}

fn color(properties: &raw::MaterialProperties, name: &str, fallback: Vec3) -> Vec3 {
    properties
        .color(name)
        .map(|color| Vec3::new(color[0], color[1], color[2]))
        .unwrap_or(fallback)
}
