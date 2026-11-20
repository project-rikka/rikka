mod animation;
mod buffer;
mod khr;
mod material;
mod mesh;
mod scene;

use std::path::Path;

use rikka_model_core::{ExportCtx, Model, Result, Stage};
use serde_json::Value;

use crate::document::{Asset, Buffer, Document, VERSION};
use crate::{glb, uri};

use buffer::Buffered;
use material::Images;
pub use material::Toon;

const GENERATOR: &str = "rikka-model";

const EMISSIVE_STRENGTH: &str = "KHR_materials_emissive_strength";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Text,
    Binary,
}

#[derive(Debug)]
pub struct Composition {
    document: Document,
    buffered: Buffered,
}

impl Composition {
    pub fn use_extension(&mut self, name: &str) {
        self.document.extensions_used.push(name.to_owned());
    }

    pub fn set_extension(&mut self, name: &str, value: Value) {
        self.document
            .extensions
            .other
            .insert(name.to_owned(), value);
    }

    #[must_use]
    pub fn set_material_extension(&mut self, material: usize, name: &str, value: Value) -> bool {
        let Some(material) = self.document.materials.get_mut(material) else {
            return false;
        };
        material.extensions.other.insert(name.to_owned(), value);
        true
    }

    #[must_use]
    pub fn set_node_extension(&mut self, node: usize, name: &str, value: Value) -> bool {
        let Some(node) = self.document.nodes.get_mut(node) else {
            return false;
        };
        node.extensions.other.insert(name.to_owned(), value);
        true
    }
}

pub(crate) fn write(
    model: &Model,
    ctx: &mut ExportCtx<'_>,
    container: Container,
) -> Result<Vec<u8>> {
    let composition = compose(model, ctx, container, Toon::Flattened)?;
    finish(composition, ctx, container)
}

pub fn compose(
    model: &Model,
    ctx: &mut ExportCtx<'_>,
    container: Container,
    toon: Toon,
) -> Result<Composition> {
    let mut document = Document {
        asset: Asset {
            version: VERSION.to_owned(),
            generator: Some(GENERATOR.to_owned()),
            ..Asset::default()
        },
        ..Document::default()
    };
    let mut buffered = Buffered::default();

    ctx.report(Stage::Writing, 0.1);
    let meshes = mesh::write(model, &mut document, &mut buffered);
    ctx.checkpoint()?;
    let skins = animation::write(model, &mut document, &mut buffered);
    scene::write(model, &mut document, &meshes, &skins);
    ctx.checkpoint()?;

    ctx.report(Stage::Resources, 0.0);
    material::write(
        model,
        &mut document,
        &mut buffered,
        ctx,
        image_storage(ctx, container),
        toon,
    )?;
    ctx.report(Stage::Resources, 1.0);

    if document
        .materials
        .iter()
        .any(|material| material.extensions.emissive_strength.is_some())
    {
        document.extensions_used.push(EMISSIVE_STRENGTH.to_owned());
    }
    khr::declare(&mut document);

    Ok(Composition { document, buffered })
}

pub fn finish(
    composition: Composition,
    ctx: &mut ExportCtx<'_>,
    container: Container,
) -> Result<Vec<u8>> {
    let Composition {
        mut document,
        buffered,
        ..
    } = composition;

    let bytes = buffered.finish(&mut document);
    if !bytes.is_empty() {
        document.buffers = vec![Buffer {
            byte_length: bytes.len(),
            uri: named_buffer(ctx, container, &bytes)?,
            name: None,
        }];
    }

    ctx.report(Stage::Writing, 0.8);
    let json = match container {
        Container::Binary => serde_json::to_vec(&document),
        Container::Text => serde_json::to_vec_pretty(&document),
    }
    .map_err(|error| rikka_model_core::Error::Encode {
        what: "glTF",
        reason: error.to_string(),
    })?;

    Ok(match container {
        Container::Binary => glb::wrap(&json, &bytes),
        Container::Text => json,
    })
}

fn image_storage(ctx: &ExportCtx<'_>, container: Container) -> Images {
    match (container, ctx.embeds()) {
        (_, false) => Images::Beside,
        (Container::Binary, true) => Images::Buffer,
        (Container::Text, true) => Images::Data,
    }
}

fn named_buffer(
    ctx: &mut ExportCtx<'_>,
    container: Container,
    bytes: &[u8],
) -> Result<Option<String>> {
    if container == Container::Binary {
        return Ok(None);
    }
    if ctx.embeds() {
        return Ok(Some(uri::embed("application/octet-stream", bytes)));
    }
    let name = format!("{}.bin", stem(ctx.name()));
    ctx.write_resource(&name, bytes)?;
    Ok(Some(uri::relative(&name)))
}

fn stem(name: &Path) -> String {
    name.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "model".to_owned())
}
