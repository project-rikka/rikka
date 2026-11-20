mod avatar;
mod materials;
mod normalize;
mod springs;

use std::collections::HashSet;

use rikka_model_core::glam::Mat3;

use rikka_gltf::{Container, Toon};
use rikka_model_core::Stage;
use rikka_model_core::Warning;
use rikka_model_core::{Error, ExportCtx, Result};
use rikka_model_core::{HumanBone, ImageId, Model, Texture};

use super::super::avatar::Vrm;
use super::super::extended_collider::ExtendedColliders;
use super::super::legacy as raw;
use super::super::spring::ColliderShape;
use super::super::vrm_legacy::VrmLegacy;

const EXPORTER_VERSION: &str = "rikka-model";

pub(crate) fn write(source: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
    validate(source, super::avatar(source)?)?;
    let mut model = prepared(source);
    rikka_model_core::convert_coordinates(&mut model, raw::COORDINATES, 1.0);
    let baked = normalize::bake(&mut model);
    if baked.changed > 0 {
        ctx.warn(Warning::Repaired(format!(
            "VRM 0.x requires nodes without rotation or scale, baked {} nodes into the rest pose",
            baked.changed
        )));
    }
    let mut vrm = super::avatar(&model)?.clone();
    rebase(&mut vrm, &baked.corrections);
    unique_material_names(&mut model, ctx);
    let thumbnail = thumbnail_texture(&mut model, vrm.meta.thumbnail);
    warn_losses(&model, &vrm, ctx);

    let legacy = model.extensions.get::<VrmLegacy>();
    let meshes = gltf_meshes(&model);
    let sources = avatar::Sources {
        model: &model,
        vrm: &vrm,
        legacy,
        meshes: &meshes,
    };
    let described = raw::Vrm {
        exporter_version: Some(EXPORTER_VERSION.to_owned()),
        spec_version: Some(raw::SPEC_VERSION.to_owned()),
        meta: Some(avatar::meta(&vrm.meta, legacy, thumbnail, ctx)),
        humanoid: Some(avatar::humanoid(&sources, ctx)),
        first_person: Some(avatar::first_person(&sources, ctx)),
        blend_shape_master: Some(avatar::blend_shape_master(&sources, ctx)),
        secondary_animation: Some(springs::write(&model, &vrm.spring_bone, ctx)),
        material_properties: materials::write(&model, ctx),
    };

    let mut composition = rikka_gltf::compose(&model, ctx, Container::Binary, Toon::Kept)?;
    ctx.report(Stage::Writing, 0.5);
    super::insert(&mut composition, raw::VRM, &described)?;
    composition.use_extension(raw::VRM);
    ctx.checkpoint()?;
    rikka_gltf::finish(composition, ctx, Container::Binary)
}

fn validate(model: &Model, vrm: &Vrm) -> Result<()> {
    let legacy = model.extensions.get::<VrmLegacy>();
    let spring_bone = &vrm.spring_bone;
    let nodes = vrm
        .humanoid
        .bones()
        .map(|(_, node)| node)
        .chain(vrm.first_person.iter().map(|annotation| annotation.node))
        .chain(
            vrm.expressions
                .iter()
                .flat_map(|expression| expression.morph_binds.iter().map(|bind| bind.node)),
        )
        .chain(spring_bone.colliders.iter().map(|collider| collider.node))
        .chain(spring_bone.springs.iter().flat_map(|spring| {
            spring
                .joints
                .iter()
                .map(|joint| joint.node)
                .chain(spring.center)
        }))
        .chain(legacy.and_then(|legacy| legacy.first_person_bone));
    for node in nodes {
        if node.index() >= model.nodes.len() {
            return Err(Error::Invalid(format!(
                "VRM data references missing node {node}"
            )));
        }
    }
    let materials = vrm
        .expressions
        .iter()
        .flat_map(|expression| {
            expression
                .material_color_binds
                .iter()
                .map(|bind| bind.material)
                .chain(
                    expression
                        .texture_transform_binds
                        .iter()
                        .map(|bind| bind.material),
                )
        })
        .chain(legacy.into_iter().flat_map(|legacy| {
            legacy.expressions.iter().flat_map(|expression| {
                expression
                    .material_values
                    .iter()
                    .map(|value| value.material)
            })
        }));
    for material in materials {
        if material.index() >= model.materials.len() {
            return Err(Error::Invalid(format!(
                "VRM data references missing material {material}"
            )));
        }
    }
    for group in &spring_bone.collider_groups {
        if let Some(collider) = group
            .colliders
            .iter()
            .find(|collider| **collider >= spring_bone.colliders.len())
        {
            return Err(Error::Invalid(format!(
                "VRM collider group references missing collider {collider}"
            )));
        }
    }
    Ok(())
}

fn prepared(source: &Model) -> Model {
    let mut model = source.clone();
    model.animations.clear();
    model.cameras.clear();
    model.lights.clear();
    for node in &mut model.nodes {
        node.camera = None;
        node.light = None;
    }
    model
}

fn rebase(vrm: &mut Vrm, corrections: &[Mat3]) {
    if let Some(head) = vrm.humanoid.bone(HumanBone::Head) {
        vrm.look_at.offset_from_head = corrections[head.index()] * vrm.look_at.offset_from_head;
    }
    for collider in &mut vrm.spring_bone.colliders {
        let linear = corrections[collider.node.index()];
        let scale = linear
            .x_axis
            .length()
            .max(linear.y_axis.length())
            .max(linear.z_axis.length());
        collider.shape = match collider.shape {
            ColliderShape::Sphere { offset, radius } => ColliderShape::Sphere {
                offset: linear * offset,
                radius: radius * scale,
            },
            ColliderShape::Capsule {
                offset,
                radius,
                tail,
            } => ColliderShape::Capsule {
                offset: linear * offset,
                radius: radius * scale,
                tail: linear * tail,
            },
        };
    }
}

fn warn_losses(model: &Model, vrm: &Vrm, ctx: &mut ExportCtx<'_>) {
    if vrm.meta.name.is_empty() {
        ctx.warn(Warning::Repaired("VRM requires a model name".to_owned()));
    }
    if !vrm.constraints.is_empty() {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 0.x has no node constraints, dropping {}",
            vrm.constraints.len()
        )));
    }
    if let Some(extended) = model
        .extensions
        .get::<ExtendedColliders>()
        .filter(|extended| !extended.is_empty())
    {
        ctx.warn(Warning::Unsupported(format!(
            "VRM 0.x has no extended colliders, writing {} as their fallback shapes",
            extended.colliders.len()
        )));
    }
}

fn unique_material_names(model: &mut Model, ctx: &mut ExportCtx<'_>) {
    let mut taken: HashSet<String> = model
        .materials
        .iter()
        .map(|material| material.name.clone())
        .collect();
    let mut seen = HashSet::new();
    for (index, material) in model.materials.iter_mut().enumerate() {
        if !material.name.is_empty() && seen.insert(material.name.clone()) {
            continue;
        }
        let base = match material.name.is_empty() {
            true => format!("material{index}"),
            false => material.name.clone(),
        };
        let name = (1..)
            .map(|suffix| format!("{base}.{suffix:03}"))
            .find(|candidate| !taken.contains(candidate))
            .unwrap_or(base);
        ctx.warn(Warning::Repaired(format!(
            "VRM 0.x addresses materials by name, renaming material {index} \"{}\" to \"{name}\"",
            material.name
        )));
        taken.insert(name.clone());
        seen.insert(name.clone());
        material.name = name;
    }
}

fn thumbnail_texture(model: &mut Model, image: Option<ImageId>) -> Option<u32> {
    let image = image.filter(|image| image.index() < model.images.len())?;
    if let Some(index) = model
        .textures
        .iter()
        .position(|texture| texture.image == image)
    {
        return Some(index as u32);
    }
    model.textures.push(Texture::new(image));
    Some((model.textures.len() - 1) as u32)
}

fn gltf_meshes(model: &Model) -> Vec<Option<u32>> {
    let mut next = 0;
    model
        .meshes
        .iter()
        .map(|mesh| {
            let written = mesh.submeshes.iter().any(|submesh| submesh.index_count > 0);
            written.then(|| {
                next += 1;
                next - 1
            })
        })
        .collect()
}
