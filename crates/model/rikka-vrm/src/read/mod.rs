pub(crate) mod animation;
mod mtoon;
mod resolve;
mod v0;
mod v1;

use rikka_gltf::{SourceDocument, TextureMap};
use rikka_model_core::Result;
use rikka_model_core::Warning;
use rikka_model_core::{HumanBone, Humanoid, Model};
use rikka_model_core::{ImportCtx, Input};
use serde::Deserialize;

use super::document as raw;
use super::legacy;

use resolve::Resolved;

const KNOWN: &[&str] = &[
    raw::VRM,
    raw::SPRING_BONE,
    raw::NODE_CONSTRAINT,
    raw::MTOON,
    legacy::VRM,
    "VRMC_materials_hdr_emissiveMultiplier",
    raw::EXTENDED_COLLIDER,
];

pub(crate) fn read(input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
    let rikka_gltf::Parts {
        mut model,
        meshes,
        texture_map,
        described: document,
    } = rikka_gltf::parts(input, KNOWN, ctx)?;

    if let Some(described) = v0::described(&document, ctx) {
        model.metadata.coordinates = legacy::COORDINATES;
        v0::materials(&described, &mut model, &texture_map, ctx);
        let (vrm, legacy) = v0::read(&described, &model, &texture_map, &meshes, ctx);
        model.extensions.insert(vrm);
        model.extensions.insert(legacy);
        return Ok(model);
    }

    materials(&document, &mut model, &texture_map, ctx);
    match v1::read(&document, &model, &texture_map, &meshes, ctx) {
        Some((vrm, extended)) => {
            model.extensions.insert(vrm);
            if !extended.is_empty() {
                model.extensions.insert(extended);
            }
        }
        None => ctx.warn(Warning::Unsupported(
            "no VRM extension, reading as glTF".to_owned(),
        )),
    }
    Ok(model)
}

fn materials(
    document: &SourceDocument,
    model: &mut Model,
    texture_map: &TextureMap,
    ctx: &mut ImportCtx<'_>,
) {
    for (index, material) in model.materials.iter_mut().enumerate() {
        let Some(value) = document.material_extension(index, raw::MTOON) else {
            continue;
        };
        match raw::MToon::deserialize(value) {
            Ok(described) => mtoon::apply(material, &described, texture_map, ctx),
            Err(error) => ctx.warn(Warning::Repaired(format!(
                "material \"{}\": invalid \"{}\", keeping glTF surface: {error}",
                material.name,
                raw::MTOON
            ))),
        }
    }
}

fn humanoid<'a>(
    bones: impl Iterator<Item = (&'a str, u32)>,
    legacy: bool,
    resolved: &Resolved<'_>,
    ctx: &mut ImportCtx<'_>,
) -> Humanoid {
    let mut humanoid = Humanoid::default();
    for (name, node) in bones {
        let bone = match legacy {
            true => legacy::bone_from_name(name),
            false => HumanBone::from_name(name),
        };
        let Some(bone) = bone else {
            ctx.warn(Warning::Unsupported(format!(
                "ignoring unknown humanoid bone \"{name}\""
            )));
            continue;
        };
        let Some(node) = resolved.node(node, &format!("{bone} bone"), ctx) else {
            continue;
        };
        humanoid.set_bone(bone, Some(node));
    }

    let missing: Vec<&str> = humanoid.missing().map(HumanBone::name).collect();
    if !missing.is_empty() {
        ctx.warn(Warning::Repaired(format!(
            "humanoid is missing required bone {}",
            missing.join(", ")
        )));
    }
    humanoid
}

fn read_extension<T: serde::de::DeserializeOwned>(
    value: Option<&serde_json::Value>,
    name: &str,
    ctx: &mut ImportCtx<'_>,
) -> Option<T> {
    match T::deserialize(value?) {
        Ok(read) => Some(read),
        Err(error) => {
            ctx.warn(Warning::Repaired(format!(
                "ignoring invalid \"{name}\": {error}"
            )));
            None
        }
    }
}
