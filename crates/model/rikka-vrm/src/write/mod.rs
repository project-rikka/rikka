mod legacy;
mod mtoon;

use std::collections::BTreeMap;

use serde_json::Value;

use rikka_gltf::{Composition, Container, Toon};
use rikka_model_core::ExportCtx;
use rikka_model_core::Stage;
use rikka_model_core::Warning;
use rikka_model_core::{Error, Requirement, Result};
use rikka_model_core::{HumanBone, Model};

use super::avatar::Vrm;
use super::constraint::{Constraint, NodeConstraint};
use super::document as raw;
use super::expression::{Expression, ExpressionName, ExpressionOverride};
use super::extended_collider::{ExtendedColliders, ExtendedShape};
use super::legacy_humanoid::LegacyHumanoid;
use super::look_at::{LookAt, RangeMap};
use super::look_at_curves::LookAtCurves;
use super::meta::Meta;
use super::spring::{ColliderShape, SpringBone};
use super::vrm_legacy::VrmLegacy;

pub(crate) use legacy::write as write_legacy;

pub(crate) fn write(model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
    let vrm = avatar(model)?;
    check(vrm, ctx);
    if let Some(legacy) = model.extensions.get::<VrmLegacy>() {
        legacy_losses(legacy, vrm, ctx);
    }
    let extended = model.extensions.get::<ExtendedColliders>();

    let mut composition = rikka_gltf::compose(model, ctx, Container::Binary, Toon::Kept)?;
    ctx.report(Stage::Writing, 0.5);
    attach(&mut composition, model, vrm, extended, ctx)?;
    ctx.checkpoint()?;
    rikka_gltf::finish(composition, ctx, Container::Binary)
}

fn avatar(model: &Model) -> Result<&Vrm> {
    model.extensions.get::<Vrm>().ok_or(Error::Unmet {
        format: "VRM",
        requirement: Requirement::Avatar,
    })
}

fn check(vrm: &Vrm, ctx: &mut ExportCtx<'_>) {
    if vrm.meta.name.is_empty() {
        ctx.warn(Warning::Repaired("VRM requires a model name".to_owned()));
    }
    let missing: Vec<&str> = vrm.humanoid.missing().map(HumanBone::name).collect();
    if !missing.is_empty() {
        ctx.warn(Warning::Repaired(format!(
            "humanoid is missing required bone {}",
            missing.join(", ")
        )));
    }
}

fn legacy_losses(legacy: &VrmLegacy, vrm: &Vrm, ctx: &mut ExportCtx<'_>) {
    let head = vrm.humanoid.bone(HumanBone::Head);
    let dropped = [
        (
            legacy.first_person_bone.is_some() && legacy.first_person_bone != head,
            "first-person bone",
        ),
        (
            legacy.humanoid != LegacyHumanoid::default(),
            "humanoid description",
        ),
        (!legacy.bone_limits.is_empty(), "human bone limits"),
        (
            legacy.look_at_curves != LookAtCurves::default(),
            "look-at curves",
        ),
        (
            legacy
                .expressions
                .iter()
                .any(|expression| !expression.material_values.is_empty()),
            "expression material values",
        ),
    ];
    let names: Vec<&str> = dropped
        .into_iter()
        .filter(|(lost, _)| *lost)
        .map(|(_, name)| name)
        .collect();
    if !names.is_empty() {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 1.0 cannot store the 0.x {}, dropping them",
            names.join(", ")
        )));
    }
}

fn attach(
    composition: &mut Composition,
    model: &Model,
    vrm: &Vrm,
    extended: Option<&ExtendedColliders>,
    ctx: &mut ExportCtx<'_>,
) -> Result<()> {
    let described = raw::Vrm {
        spec_version: raw::SPEC_VERSION.to_owned(),
        meta: meta(&vrm.meta),
        humanoid: raw::Humanoid {
            human_bones: vrm
                .humanoid
                .bones()
                .map(|(bone, node)| (bone.name().to_owned(), raw::HumanBone { node: node.0 }))
                .collect(),
        },
        first_person: match vrm.first_person.is_empty() {
            true => None,
            false => Some(raw::FirstPerson {
                mesh_annotations: vrm
                    .first_person
                    .iter()
                    .map(|annotation| raw::MeshAnnotation {
                        node: annotation.node.0,
                        kind: annotation.kind.name().to_owned(),
                    })
                    .collect(),
            }),
        },
        look_at: Some(look_at(&vrm.look_at)),
        expressions: expressions(&vrm.expressions),
    };

    insert(composition, raw::VRM, &described)?;
    composition.use_extension(raw::VRM);

    if !vrm.spring_bone.is_empty() {
        let (described, extends) = spring_bone(&vrm.spring_bone, extended, ctx)?;
        insert(composition, raw::SPRING_BONE, &described)?;
        composition.use_extension(raw::SPRING_BONE);
        if extends {
            composition.use_extension(raw::EXTENDED_COLLIDER);
        }
    } else if extended.is_some_and(|extended| !extended.is_empty()) {
        ctx.warn(Warning::Repaired(
            "ignoring extended colliders without spring bone colliders".to_owned(),
        ));
    }
    if !vrm.constraints.is_empty() {
        constraints(composition, &vrm.constraints)?;
        composition.use_extension(raw::NODE_CONSTRAINT);
    }
    if mtoon::used(model) {
        mtoon::write(model, composition)?;
        composition.use_extension(raw::MTOON);
    }
    Ok(())
}

fn insert<T: serde::Serialize>(
    composition: &mut Composition,
    name: &str,
    extension: &T,
) -> Result<()> {
    let value = encode("a VRM extension", extension)?;
    composition.set_extension(name, value);
    Ok(())
}

fn encode<T: serde::Serialize>(what: &'static str, value: &T) -> Result<Value> {
    serde_json::to_value(value).map_err(|error| Error::Encode {
        what,
        reason: error.to_string(),
    })
}

fn meta(meta: &Meta) -> raw::Meta {
    raw::Meta {
        name: meta.name.clone(),
        version: meta.version.clone(),
        authors: meta.authors.clone(),
        copyright_information: meta.copyright.clone(),
        contact_information: meta.contact.clone(),
        references: meta.references.clone(),
        third_party_licenses: meta.third_party_licenses.clone(),
        thumbnail_image: meta.thumbnail.map(|image| image.0),
        license_url: meta.license_url.clone(),
        other_license_url: meta.other_license_url.clone(),
        avatar_permission: Some(meta.avatar_permission.name().to_owned()),
        commercial_usage: Some(meta.commercial_usage.name().to_owned()),
        credit_notation: Some(meta.credit_notation.name().to_owned()),
        modification: Some(meta.modification.name().to_owned()),
        allow_redistribution: Some(meta.allow_redistribution),
        allow_excessively_violent_usage: Some(meta.allow_excessively_violent_usage),
        allow_excessively_sexual_usage: Some(meta.allow_excessively_sexual_usage),
        allow_political_or_religious_usage: Some(meta.allow_political_or_religious_usage),
        allow_antisocial_or_hate_usage: Some(meta.allow_antisocial_or_hate_usage),
    }
}

fn look_at(look_at: &LookAt) -> raw::LookAt {
    let map = |range: RangeMap| raw::RangeMap {
        input_max_value: Some(range.input_max_degrees),
        output_scale: Some(range.output_scale),
    };
    raw::LookAt {
        offset_from_head_bone: Some(look_at.offset_from_head.to_array()),
        kind: Some(look_at.kind.name().to_owned()),
        range_map_horizontal_inner: Some(map(look_at.horizontal_inner)),
        range_map_horizontal_outer: Some(map(look_at.horizontal_outer)),
        range_map_vertical_down: Some(map(look_at.vertical_down)),
        range_map_vertical_up: Some(map(look_at.vertical_up)),
    }
}

fn expressions(expressions: &[Expression]) -> Option<raw::Expressions> {
    if expressions.is_empty() {
        return None;
    }
    let mut preset = BTreeMap::new();
    let mut custom = BTreeMap::new();
    for expression in expressions {
        let described = raw::Expression {
            morph_target_binds: expression
                .morph_binds
                .iter()
                .map(|bind| raw::MorphTargetBind {
                    node: bind.node.0,
                    index: bind.index,
                    weight: bind.weight,
                })
                .collect(),
            material_color_binds: expression
                .material_color_binds
                .iter()
                .map(|bind| raw::MaterialColorBind {
                    material: bind.material.0,
                    kind: bind.color.name().to_owned(),
                    target_value: bind.value.to_array(),
                })
                .collect(),
            texture_transform_binds: expression
                .texture_transform_binds
                .iter()
                .map(|bind| raw::TextureTransformBind {
                    material: bind.material.0,
                    scale: Some(bind.scale.to_array()),
                    offset: Some(bind.offset.to_array()),
                })
                .collect(),
            is_binary: Some(expression.is_binary),
            override_blink: overridden(expression.override_blink),
            override_look_at: overridden(expression.override_look_at),
            override_mouth: overridden(expression.override_mouth),
        };
        match &expression.name {
            ExpressionName::Preset(preset_name) => {
                preset.insert(preset_name.name().to_owned(), described)
            }
            ExpressionName::Custom(name) => custom.insert(name.clone(), described),
        };
    }
    Some(raw::Expressions { preset, custom })
}

fn overridden(mode: ExpressionOverride) -> Option<String> {
    match mode {
        ExpressionOverride::None => None,
        mode => Some(mode.name().to_owned()),
    }
}

fn spring_bone(
    bone: &SpringBone,
    extended: Option<&ExtendedColliders>,
    ctx: &mut ExportCtx<'_>,
) -> Result<(raw::SpringBone, bool)> {
    if let Some(extended) = extended {
        for orphan in extended
            .colliders
            .iter()
            .filter(|extended| extended.collider >= bone.colliders.len())
        {
            ctx.warn(Warning::Repaired(format!(
                "ignoring extended collider of missing collider {}",
                orphan.collider
            )));
        }
    }
    let mut colliders = Vec::with_capacity(bone.colliders.len());
    let mut extends = false;
    for (index, collider) in bone.colliders.iter().enumerate() {
        let extensions = match extended.and_then(|extended| extended.shape_of(index)) {
            Some(shape) => {
                extends = true;
                Some(raw::ColliderExtensions {
                    extended_collider: Some(encode(
                        "an extended collider",
                        &extended_collider(shape),
                    )?),
                })
            }
            None => None,
        };
        colliders.push(raw::Collider {
            node: collider.node.0,
            shape: match collider.shape {
                ColliderShape::Sphere { offset, radius } => raw::ColliderShape {
                    sphere: Some(raw::Sphere {
                        offset: Some(offset.to_array()),
                        radius: Some(radius),
                    }),
                    capsule: None,
                },
                ColliderShape::Capsule {
                    offset,
                    radius,
                    tail,
                } => raw::ColliderShape {
                    sphere: None,
                    capsule: Some(raw::Capsule {
                        offset: Some(offset.to_array()),
                        radius: Some(radius),
                        tail: Some(tail.to_array()),
                    }),
                },
            },
            extensions,
        });
    }
    let described = raw::SpringBone {
        spec_version: raw::SPEC_VERSION.to_owned(),
        colliders,
        collider_groups: bone
            .collider_groups
            .iter()
            .map(|group| raw::ColliderGroup {
                name: Some(group.name.clone()).filter(|name| !name.is_empty()),
                colliders: group
                    .colliders
                    .iter()
                    .map(|collider| *collider as u32)
                    .collect(),
            })
            .collect(),
        springs: bone
            .springs
            .iter()
            .map(|spring| raw::Spring {
                name: Some(spring.name.clone()).filter(|name| !name.is_empty()),
                joints: spring
                    .joints
                    .iter()
                    .map(|joint| raw::Joint {
                        node: joint.node.0,
                        hit_radius: Some(joint.hit_radius),
                        stiffness: Some(joint.stiffness),
                        gravity_power: Some(joint.gravity_power),
                        gravity_dir: Some(joint.gravity_direction.to_array()),
                        drag_force: Some(joint.drag),
                    })
                    .collect(),
                collider_groups: spring
                    .collider_groups
                    .iter()
                    .map(|group| *group as u32)
                    .collect(),
                center: spring.center.map(|center| center.0),
            })
            .collect(),
    };
    Ok((described, extends))
}

fn extended_collider(shape: &ExtendedShape) -> raw::ExtendedCollider {
    let described = match *shape {
        ExtendedShape::Sphere {
            offset,
            radius,
            inside,
        } => raw::ExtendedShape {
            sphere: Some(raw::ExtendedSphere {
                offset: Some(offset.to_array()),
                radius: Some(radius),
                inside: Some(inside),
            }),
            ..raw::ExtendedShape::default()
        },
        ExtendedShape::Capsule {
            offset,
            radius,
            tail,
            inside,
        } => raw::ExtendedShape {
            capsule: Some(raw::ExtendedCapsule {
                offset: Some(offset.to_array()),
                radius: Some(radius),
                tail: Some(tail.to_array()),
                inside: Some(inside),
            }),
            ..raw::ExtendedShape::default()
        },
        ExtendedShape::Plane { offset, normal } => raw::ExtendedShape {
            plane: Some(raw::ExtendedPlane {
                offset: Some(offset.to_array()),
                normal: Some(normal.to_array()),
            }),
            ..raw::ExtendedShape::default()
        },
    };
    raw::ExtendedCollider {
        spec_version: raw::SPEC_VERSION.to_owned(),
        shape: Some(described),
    }
}

fn constraints(composition: &mut Composition, constraints: &[NodeConstraint]) -> Result<()> {
    for node_constraint in constraints {
        let constraint = match node_constraint.constraint {
            Constraint::Roll {
                source,
                axis,
                weight,
            } => raw::Constraint {
                roll: Some(raw::RollConstraint {
                    source: source.0,
                    roll_axis: axis.name().to_owned(),
                    weight: Some(weight),
                }),
                aim: None,
                rotation: None,
            },
            Constraint::Aim {
                source,
                axis,
                weight,
            } => raw::Constraint {
                roll: None,
                aim: Some(raw::AimConstraint {
                    source: source.0,
                    aim_axis: axis.name().to_owned(),
                    weight: Some(weight),
                }),
                rotation: None,
            },
            Constraint::Rotation { source, weight } => raw::Constraint {
                roll: None,
                aim: None,
                rotation: Some(raw::RotationConstraint {
                    source: source.0,
                    weight: Some(weight),
                }),
            },
        };
        let described = raw::NodeConstraint {
            spec_version: raw::SPEC_VERSION.to_owned(),
            constraint,
        };
        let value = encode("a node constraint", &described)?;
        if !composition.set_node_extension(
            node_constraint.node.index(),
            raw::NODE_CONSTRAINT,
            value,
        ) {
            return Err(Error::Invalid(
                "node constraint targets a node missing from the document".to_owned(),
            ));
        }
    }
    Ok(())
}
