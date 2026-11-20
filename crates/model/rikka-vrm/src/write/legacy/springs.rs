use rikka_model_core::glam::Vec3;

use rikka_model_core::ExportCtx;
use rikka_model_core::Warning;
use rikka_model_core::{Model, NodeId};

use super::super::super::legacy as raw;
use super::super::super::spring::{ColliderShape, Spring, SpringBone, SpringJoint};

const CAPSULE_SPHERE_LIMIT: usize = 64;

const DEVIATION: f32 = 1e-6;

#[derive(Clone, Debug, PartialEq)]
struct Settings {
    comment: String,
    stiffness: f32,
    gravity_power: f32,
    gravity_direction: Vec3,
    drag: f32,
    hit_radius: f32,
    center: Option<NodeId>,
    collider_groups: Vec<u32>,
}

pub(super) fn write(
    model: &Model,
    bone: &SpringBone,
    ctx: &mut ExportCtx<'_>,
) -> raw::SecondaryAnimation {
    let (collider_groups, group_map) = collider_groups(bone, ctx);
    let mut bone_groups: Vec<(Settings, Vec<u32>)> = Vec::new();
    let mut uneven = Vec::new();
    let mut branching = Vec::new();
    for spring in &bone.springs {
        let Some(head) = spring.joints.first() else {
            continue;
        };
        if !is_uniform(spring) {
            uneven.push(spring.name.clone());
        }
        if !follows_first_children(model, spring) {
            branching.push(spring.name.clone());
        }
        let settings = Settings {
            comment: spring.name.clone(),
            stiffness: head.stiffness,
            gravity_power: head.gravity_power,
            gravity_direction: head.gravity_direction,
            drag: head.drag,
            hit_radius: head.hit_radius,
            center: spring.center,
            collider_groups: spring
                .collider_groups
                .iter()
                .filter_map(|group| group_map.get(*group))
                .flatten()
                .copied()
                .collect(),
        };
        match bone_groups.last_mut() {
            Some((last, roots)) if *last == settings => roots.push(head.node.0),
            _ => bone_groups.push((settings, vec![head.node.0])),
        }
    }
    if !uneven.is_empty() {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 0.x applies one setting to a whole spring, using the first joint of {}",
            quoted(&uneven)
        )));
    }
    if !branching.is_empty() {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 0.x swings every descendant of a spring root, joints of {} differ",
            quoted(&branching)
        )));
    }

    raw::SecondaryAnimation {
        bone_groups: bone_groups
            .into_iter()
            .map(|(settings, bones)| raw::BoneGroup {
                comment: Some(settings.comment),
                stiffiness: Some(settings.stiffness),
                gravity_power: Some(settings.gravity_power),
                gravity_dir: Some(raw::Vector3::from_gltf(settings.gravity_direction)),
                drag_force: Some(settings.drag),
                center: Some(settings.center.map_or(-1, |center| center.0 as i32)),
                hit_radius: Some(settings.hit_radius),
                bones,
                collider_groups: settings.collider_groups,
            })
            .collect(),
        collider_groups,
    }
}

fn collider_groups(
    bone: &SpringBone,
    ctx: &mut ExportCtx<'_>,
) -> (Vec<raw::ColliderGroup>, Vec<Vec<u32>>) {
    let mut written: Vec<raw::ColliderGroup> = Vec::new();
    let mut group_map = Vec::with_capacity(bone.collider_groups.len());
    let mut capsules = 0;
    for group in &bone.collider_groups {
        let mut by_node: Vec<(NodeId, Vec<raw::Collider>)> = Vec::new();
        for collider in group
            .colliders
            .iter()
            .filter_map(|collider| bone.colliders.get(*collider))
        {
            if matches!(collider.shape, ColliderShape::Capsule { .. }) {
                capsules += 1;
            }
            let spheres =
                spheres(collider.shape)
                    .into_iter()
                    .map(|(offset, radius)| raw::Collider {
                        offset: Some(raw::Vector3::from_gltf(offset)),
                        radius: Some(radius),
                    });
            match by_node.iter_mut().find(|(node, _)| *node == collider.node) {
                Some((_, colliders)) => colliders.extend(spheres),
                None => by_node.push((collider.node, spheres.collect())),
            }
        }
        let mut indices = Vec::with_capacity(by_node.len());
        for (node, colliders) in by_node {
            indices.push(written.len() as u32);
            written.push(raw::ColliderGroup {
                node: Some(node.0),
                colliders,
            });
        }
        group_map.push(indices);
    }
    if capsules > 0 {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 0.x holds sphere colliders alone, {capsules} capsule colliders become sphere chains"
        )));
    }
    (written, group_map)
}

fn spheres(shape: ColliderShape) -> Vec<(Vec3, f32)> {
    match shape {
        ColliderShape::Sphere { offset, radius } => vec![(offset, radius)],
        ColliderShape::Capsule {
            offset,
            radius,
            tail,
        } => {
            let length = offset.distance(tail);
            if length <= DEVIATION {
                return vec![(offset, radius)];
            }
            let count = match radius > 0.0 {
                true => ((length / radius).ceil() as usize + 1).clamp(2, CAPSULE_SPHERE_LIMIT),
                false => 2,
            };
            (0..count)
                .map(|step| {
                    let along = step as f32 / (count - 1) as f32;
                    (offset.lerp(tail, along), radius)
                })
                .collect()
        }
    }
}

fn is_uniform(spring: &Spring) -> bool {
    let swinging = &spring.joints[..spring.joints.len().saturating_sub(1).max(1)];
    let head = &swinging[0];
    swinging.iter().all(|joint| same_settings(joint, head))
}

fn same_settings(joint: &SpringJoint, head: &SpringJoint) -> bool {
    (joint.stiffness - head.stiffness).abs() <= DEVIATION
        && (joint.gravity_power - head.gravity_power).abs() <= DEVIATION
        && joint
            .gravity_direction
            .abs_diff_eq(head.gravity_direction, DEVIATION)
        && (joint.drag - head.drag).abs() <= DEVIATION
        && (joint.hit_radius - head.hit_radius).abs() <= DEVIATION
}

fn follows_first_children(model: &Model, spring: &Spring) -> bool {
    let mut cursor = spring.joints.first().map(|joint| joint.node);
    for joint in &spring.joints {
        if cursor != Some(joint.node) {
            return false;
        }
        cursor = model.node(joint.node).children.first().copied();
    }
    cursor.is_none()
}

fn quoted(names: &[String]) -> String {
    names
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ")
}
