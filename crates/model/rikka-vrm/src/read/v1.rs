use rikka_model_core::glam::{Vec2, Vec3, Vec4};

use rikka_gltf::SourceDocument;
use rikka_gltf::TextureMap;
use rikka_model_core::ImportCtx;
use rikka_model_core::MeshId;
use rikka_model_core::Model;
use rikka_model_core::NodeId;
use rikka_model_core::Warning;

use super::super::avatar::{Vrm, VrmVersion};
use super::super::constraint::{AimAxis, Constraint, NodeConstraint, RollAxis};
use super::super::document as raw;
use super::super::expression::{
    Expression, ExpressionName, ExpressionOverride, ExpressionPreset, MaterialColor,
    MaterialColorBind, MorphBind, TextureTransformBind,
};
use super::super::extended_collider::{ExtendedCollider, ExtendedColliders, ExtendedShape};
use super::super::first_person::{FirstPersonKind, MeshAnnotation};
use super::super::look_at::{LookAt, LookAtKind, RangeMap};
use super::super::meta::{AvatarPermission, CommercialUsage, CreditNotation, Meta, Modification};
use super::super::spring::{
    Collider, ColliderGroup, ColliderShape, Spring, SpringBone, SpringJoint,
};
use super::resolve::Resolved;

pub(super) fn read(
    document: &SourceDocument,
    model: &Model,
    texture_map: &TextureMap,
    meshes: &[Option<MeshId>],
    ctx: &mut ImportCtx<'_>,
) -> Option<(Vrm, ExtendedColliders)> {
    let described: raw::Vrm = super::read_extension(document.extension(raw::VRM), raw::VRM, ctx)?;
    if !described.spec_version.starts_with("1.") {
        ctx.warn(Warning::Unsupported(format!(
            "VRM {} is read as 1.0",
            described.spec_version
        )));
    }

    let resolved = Resolved::new(model, meshes);
    let (spring_bone, extended) = spring_bone(document, &resolved, ctx);
    let vrm = Vrm {
        version: VrmVersion::V1,
        meta: meta(&described.meta, texture_map),
        humanoid: super::humanoid(
            described
                .humanoid
                .human_bones
                .iter()
                .map(|(name, bone)| (name.as_str(), bone.node)),
            false,
            &resolved,
            ctx,
        ),
        first_person: first_person(described.first_person.as_ref(), &resolved, ctx),
        look_at: look_at(described.look_at.as_ref()),
        expressions: expressions(described.expressions.as_ref(), &resolved, ctx),
        spring_bone,
        constraints: constraints(document, &resolved, ctx),
    };
    Some((vrm, extended))
}

fn meta(described: &raw::Meta, texture_map: &TextureMap) -> Meta {
    Meta {
        name: described.name.clone(),
        version: described.version.clone(),
        authors: described.authors.clone(),
        copyright: described.copyright_information.clone(),
        contact: described.contact_information.clone(),
        references: described.references.clone(),
        third_party_licenses: described.third_party_licenses.clone(),
        thumbnail: described
            .thumbnail_image
            .and_then(|image| texture_map.image(image)),
        license_url: described.license_url.clone(),
        other_license_url: described.other_license_url.clone(),
        avatar_permission: setting(
            described.avatar_permission.as_deref(),
            AvatarPermission::from_name,
        ),
        commercial_usage: setting(
            described.commercial_usage.as_deref(),
            CommercialUsage::from_name,
        ),
        credit_notation: setting(
            described.credit_notation.as_deref(),
            CreditNotation::from_name,
        ),
        modification: setting(described.modification.as_deref(), Modification::from_name),
        allow_redistribution: described.allow_redistribution.unwrap_or(false),
        allow_excessively_violent_usage: described.allow_excessively_violent_usage.unwrap_or(false),
        allow_excessively_sexual_usage: described.allow_excessively_sexual_usage.unwrap_or(false),
        allow_political_or_religious_usage: described
            .allow_political_or_religious_usage
            .unwrap_or(false),
        allow_antisocial_or_hate_usage: described.allow_antisocial_or_hate_usage.unwrap_or(false),
    }
}

fn setting<T: Default>(name: Option<&str>, from_name: fn(&str) -> Option<T>) -> T {
    name.and_then(from_name).unwrap_or_default()
}

fn first_person(
    described: Option<&raw::FirstPerson>,
    resolved: &Resolved<'_>,
    ctx: &mut ImportCtx<'_>,
) -> Vec<MeshAnnotation> {
    let Some(described) = described else {
        return Vec::new();
    };
    described
        .mesh_annotations
        .iter()
        .filter_map(|annotation| {
            Some(MeshAnnotation {
                node: resolved.node(annotation.node, "first-person annotation", ctx)?,
                kind: FirstPersonKind::from_name(&annotation.kind).unwrap_or_default(),
            })
        })
        .collect()
}

fn look_at(described: Option<&raw::LookAt>) -> LookAt {
    let Some(described) = described else {
        return LookAt::default();
    };
    let kind = described
        .kind
        .as_deref()
        .and_then(LookAtKind::from_name)
        .unwrap_or_default();
    let map = |range_map: Option<&raw::RangeMap>| match range_map {
        Some(range_map) => RangeMap::new(
            range_map.input_max_value.unwrap_or(90.0),
            range_map
                .output_scale
                .unwrap_or(kind.default_output_scale()),
        ),
        None => RangeMap::new(90.0, kind.default_output_scale()),
    };
    LookAt {
        offset_from_head: described
            .offset_from_head_bone
            .map(Vec3::from_array)
            .unwrap_or(LookAt::for_kind(kind).offset_from_head),
        kind,
        horizontal_inner: map(described.range_map_horizontal_inner.as_ref()),
        horizontal_outer: map(described.range_map_horizontal_outer.as_ref()),
        vertical_down: map(described.range_map_vertical_down.as_ref()),
        vertical_up: map(described.range_map_vertical_up.as_ref()),
    }
}

fn expressions(
    described: Option<&raw::Expressions>,
    resolved: &Resolved<'_>,
    ctx: &mut ImportCtx<'_>,
) -> Vec<Expression> {
    let Some(described) = described else {
        return Vec::new();
    };
    let presets = described
        .preset
        .iter()
        .filter_map(|(name, described_expression)| {
            let preset = ExpressionPreset::from_name(name)?;
            Some((ExpressionName::Preset(preset), described_expression))
        });
    let custom = described.custom.iter().map(|(name, described_expression)| {
        (ExpressionName::Custom(name.clone()), described_expression)
    });

    presets
        .chain(custom)
        .map(|(name, described_expression)| expression(name, described_expression, resolved, ctx))
        .collect()
}

fn expression(
    name: ExpressionName,
    described: &raw::Expression,
    resolved: &Resolved<'_>,
    ctx: &mut ImportCtx<'_>,
) -> Expression {
    let what = format!("expression \"{name}\"");
    Expression {
        morph_binds: described
            .morph_target_binds
            .iter()
            .filter_map(|bind| {
                let node = resolved.node(bind.node, &what, ctx)?;
                resolved.morph(node, bind.index, &what, ctx)?;
                Some(MorphBind {
                    node,
                    index: bind.index,
                    weight: bind.weight,
                })
            })
            .collect(),
        material_color_binds: described
            .material_color_binds
            .iter()
            .filter_map(|bind| {
                Some(MaterialColorBind {
                    material: resolved.material(bind.material, &what, ctx)?,
                    color: MaterialColor::from_name(&bind.kind)?,
                    value: Vec4::from_array(bind.target_value),
                })
            })
            .collect(),
        texture_transform_binds: described
            .texture_transform_binds
            .iter()
            .filter_map(|bind| {
                Some(TextureTransformBind {
                    material: resolved.material(bind.material, &what, ctx)?,
                    scale: Vec2::from_array(bind.scale.unwrap_or([1.0; 2])),
                    offset: Vec2::from_array(bind.offset.unwrap_or([0.0; 2])),
                })
            })
            .collect(),
        is_binary: described.is_binary.unwrap_or(false),
        override_blink: setting(
            described.override_blink.as_deref(),
            ExpressionOverride::from_name,
        ),
        override_look_at: setting(
            described.override_look_at.as_deref(),
            ExpressionOverride::from_name,
        ),
        override_mouth: setting(
            described.override_mouth.as_deref(),
            ExpressionOverride::from_name,
        ),
        name,
    }
}

fn spring_bone(
    document: &SourceDocument,
    resolved: &Resolved<'_>,
    ctx: &mut ImportCtx<'_>,
) -> (SpringBone, ExtendedColliders) {
    let Some(described) = super::read_extension::<raw::SpringBone>(
        document.extension(raw::SPRING_BONE),
        raw::SPRING_BONE,
        ctx,
    ) else {
        return (SpringBone::default(), ExtendedColliders::default());
    };

    let mut bone = SpringBone::default();
    let mut extended = ExtendedColliders::default();
    let mut collider_slots = Vec::with_capacity(described.colliders.len());
    for collider in &described.colliders {
        let Some(node) = resolved.node(collider.node, "spring bone collider", ctx) else {
            collider_slots.push(None);
            continue;
        };
        let extended_shape = collider
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.extended_collider.as_ref())
            .and_then(|value| {
                super::read_extension::<raw::ExtendedCollider>(
                    Some(value),
                    raw::EXTENDED_COLLIDER,
                    ctx,
                )
            })
            .and_then(|described| extended_collider_shape(described.shape.as_ref()?));
        let Some(shape) = collider_shape(&collider.shape)
            .or_else(|| extended_shape.map(|shape| shape.fallback()))
        else {
            ctx.warn(Warning::Repaired(
                "skipping spring bone collider without shape".to_owned(),
            ));
            collider_slots.push(None);
            continue;
        };
        if let Some(shape) = extended_shape {
            extended.colliders.push(ExtendedCollider {
                collider: bone.colliders.len(),
                shape,
            });
        }
        collider_slots.push(Some(bone.colliders.len()));
        bone.colliders.push(Collider { node, shape });
    }

    let mut groups = Vec::with_capacity(described.collider_groups.len());
    for group in &described.collider_groups {
        groups.push(Some(bone.collider_groups.len()));
        bone.collider_groups.push(ColliderGroup {
            name: group.name.clone().unwrap_or_default(),
            colliders: group
                .colliders
                .iter()
                .filter_map(|collider| collider_slots.get(*collider as usize).copied().flatten())
                .collect(),
        });
    }

    bone.springs = described
        .springs
        .iter()
        .map(|spring| Spring {
            name: spring.name.clone().unwrap_or_default(),
            joints: spring
                .joints
                .iter()
                .filter_map(|joint| {
                    Some(SpringJoint {
                        node: resolved.node(joint.node, "spring bone joint", ctx)?,
                        hit_radius: joint.hit_radius.unwrap_or(0.0),
                        stiffness: joint.stiffness.unwrap_or(1.0),
                        gravity_power: joint.gravity_power.unwrap_or(0.0),
                        gravity_direction: joint
                            .gravity_dir
                            .map(Vec3::from_array)
                            .unwrap_or(Vec3::NEG_Y),
                        drag: joint.drag_force.unwrap_or(0.5),
                    })
                })
                .collect(),
            collider_groups: spring
                .collider_groups
                .iter()
                .filter_map(|group| groups.get(*group as usize).copied().flatten())
                .collect(),
            center: spring
                .center
                .and_then(|center| resolved.node(center, "spring bone centre", ctx)),
        })
        .filter(|spring| !spring.joints.is_empty())
        .collect();
    (bone, extended)
}

fn collider_shape(described: &raw::ColliderShape) -> Option<ColliderShape> {
    if let Some(capsule) = &described.capsule {
        return Some(ColliderShape::Capsule {
            offset: Vec3::from_array(capsule.offset.unwrap_or([0.0; 3])),
            radius: capsule.radius.unwrap_or(0.0),
            tail: Vec3::from_array(capsule.tail.unwrap_or([0.0; 3])),
        });
    }
    let sphere = described.sphere.as_ref()?;
    Some(ColliderShape::Sphere {
        offset: Vec3::from_array(sphere.offset.unwrap_or([0.0; 3])),
        radius: sphere.radius.unwrap_or(0.0),
    })
}

fn extended_collider_shape(described: &raw::ExtendedShape) -> Option<ExtendedShape> {
    let point = |value: Option<[f32; 3]>| Vec3::from_array(value.unwrap_or([0.0; 3]));
    if let Some(sphere) = &described.sphere {
        return Some(ExtendedShape::Sphere {
            offset: point(sphere.offset),
            radius: sphere.radius.unwrap_or(0.0),
            inside: sphere.inside.unwrap_or(false),
        });
    }
    if let Some(capsule) = &described.capsule {
        return Some(ExtendedShape::Capsule {
            offset: point(capsule.offset),
            radius: capsule.radius.unwrap_or(0.0),
            tail: point(capsule.tail),
            inside: capsule.inside.unwrap_or(false),
        });
    }
    let plane = described.plane.as_ref()?;
    Some(ExtendedShape::Plane {
        offset: point(plane.offset),
        normal: plane
            .normal
            .map(Vec3::from_array)
            .unwrap_or(Vec3::Z)
            .normalize_or(Vec3::Z),
    })
}

fn constraints(
    document: &SourceDocument,
    resolved: &Resolved<'_>,
    ctx: &mut ImportCtx<'_>,
) -> Vec<NodeConstraint> {
    let mut constraints = Vec::new();
    for (index, value) in document.node_extensions(raw::NODE_CONSTRAINT) {
        let Some(described) =
            super::read_extension::<raw::NodeConstraint>(Some(value), raw::NODE_CONSTRAINT, ctx)
        else {
            continue;
        };
        let what = "node constraint";
        let raw_constraint = &described.constraint;
        let constraint = if let Some(roll) = &raw_constraint.roll {
            Some(Constraint::Roll {
                source: match resolved.node(roll.source, what, ctx) {
                    Some(source) => source,
                    None => continue,
                },
                axis: RollAxis::from_name(&roll.roll_axis).unwrap_or_default(),
                weight: roll.weight.unwrap_or(1.0),
            })
        } else if let Some(aim) = &raw_constraint.aim {
            Some(Constraint::Aim {
                source: match resolved.node(aim.source, what, ctx) {
                    Some(source) => source,
                    None => continue,
                },
                axis: AimAxis::from_name(&aim.aim_axis).unwrap_or_default(),
                weight: aim.weight.unwrap_or(1.0),
            })
        } else {
            raw_constraint.rotation.as_ref().and_then(|rotation| {
                Some(Constraint::Rotation {
                    source: resolved.node(rotation.source, what, ctx)?,
                    weight: rotation.weight.unwrap_or(1.0),
                })
            })
        };

        let node = NodeId(index as u32);
        match constraint {
            Some(constraint) if constraint.source() != node => {
                constraints.push(NodeConstraint { node, constraint });
            }
            Some(_) => ctx.warn(Warning::Repaired(
                "skipping self-referencing node constraint".to_owned(),
            )),
            None => ctx.warn(Warning::Repaired(
                "skipping node constraint without kind".to_owned(),
            )),
        }
    }
    constraints
}
