mod accessor;
mod animation;
mod buffers;
pub(crate) mod material;
mod mesh;
mod scene;

pub use material::TextureMap;

use crate::document::Document;
use crate::glb::Chunks;
use rikka_model_core::MeshId;
use rikka_model_core::Model;
use rikka_model_core::ModelBuilder;
use rikka_model_core::Stage;
use rikka_model_core::Warning;
use rikka_model_core::{Error, Result};
use rikka_model_core::{ImportCtx, Input};
use serde_json::Value;

use accessor::Accessors;
use buffers::Buffers;

const SUPPORTED: &[&str] = &[
    "KHR_lights_punctual",
    "KHR_materials_anisotropy",
    "KHR_materials_clearcoat",
    "KHR_materials_dispersion",
    "KHR_materials_emissive_strength",
    "KHR_materials_ior",
    "KHR_materials_iridescence",
    "KHR_materials_sheen",
    "KHR_materials_specular",
    "KHR_materials_transmission",
    "KHR_materials_unlit",
    "KHR_materials_volume",
    "KHR_mesh_quantization",
    "KHR_texture_transform",
];

#[derive(Debug)]
pub struct Parts {
    pub model: Model,
    pub meshes: Vec<Option<MeshId>>,
    pub texture_map: TextureMap,
    pub described: SourceDocument,
}

#[derive(Debug)]
pub struct SourceDocument(Document);

impl SourceDocument {
    #[must_use]
    pub fn extension(&self, name: &str) -> Option<&Value> {
        self.0.extensions.other.get(name)
    }

    #[must_use]
    pub fn material_extension(&self, material: usize, name: &str) -> Option<&Value> {
        self.0.materials.get(material)?.extensions.other.get(name)
    }

    pub fn node_extensions<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = (usize, &'a Value)> {
        self.0
            .nodes
            .iter()
            .enumerate()
            .filter_map(move |(index, node)| Some((index, node.extensions.other.get(name)?)))
    }
}

pub(crate) fn read(input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
    Ok(parts(input, &[], ctx)?.model)
}

pub fn parts(input: &Input<'_>, also_read: &[&str], ctx: &mut ImportCtx<'_>) -> Result<Parts> {
    let chunks = Chunks::split(input.bytes())?;
    let document: Document = serde_json::from_slice(chunks.json)
        .map_err(|error| Error::Damaged(format!("invalid glTF JSON: {error}")))?;
    supported(&document, also_read, ctx)?;
    ctx.report(Stage::Parsing, 0.1);

    let buffers = Buffers::load(&document, chunks.binary, ctx)?;
    let accessors = Accessors::new(&document, &buffers, ctx)?;
    ctx.report(Stage::Parsing, 0.3);

    let mut builder = ModelBuilder::new();
    if let Some(generator) = &document.asset.generator {
        builder = builder.generator(generator.clone());
    }
    let texture_map = material::read(&document, &buffers, also_read, &mut builder, ctx)?;
    let meshes = mesh::read(&document, &accessors, &mut builder, ctx)?;
    scene::read(&document, &meshes, &mut builder, ctx)?;
    animation::read_skins(&document, &accessors, &mut builder, ctx)?;
    animation::read_animations(&document, &accessors, &mut builder, ctx)?;
    ctx.report(Stage::Parsing, 1.0);

    Ok(Parts {
        model: builder.finish()?,
        meshes,
        texture_map,
        described: SourceDocument(document),
    })
}

fn supported(document: &Document, also_read: &[&str], ctx: &mut ImportCtx<'_>) -> Result<()> {
    let known = |name: &str| SUPPORTED.contains(&name) || also_read.contains(&name);
    let version = &document.asset.version;
    if !version.starts_with("2.") {
        return Err(Error::UnsupportedVersion {
            format: "glTF",
            version: version.clone(),
        });
    }
    if let Some(least) = &document.asset.min_version
        && !least.starts_with("2.")
    {
        return Err(Error::UnsupportedVersion {
            format: "glTF",
            version: least.clone(),
        });
    }

    let missing: Vec<String> = document
        .extensions_required
        .iter()
        .filter(|name| !known(name))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(Error::RequiredExtensions(missing));
    }

    for name in document.extensions.other.keys().filter(|name| !known(name)) {
        ctx.warn(Warning::Unsupported(format!(
            "ignoring unsupported extension \"{name}\""
        )));
    }
    Ok(())
}
