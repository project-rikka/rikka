use rikka_model_core::glam::Vec3;

use crate::document::{Node, Value};
use crate::write::{Composer, named, number, prop, vector};
use rikka_model_core::ExportCtx;
use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::Stage;
use rikka_model_core::TextureId;
use rikka_model_core::path;
use rikka_model_core::{MaterialId, MaterialTextures, Shading};

fn slots(textures: &MaterialTextures) -> [(&'static str, Option<TextureId>); 5] {
    [
        ("DiffuseColor", textures.base_color),
        ("NormalMap", textures.normal),
        ("EmissiveColor", textures.emissive),
        ("roughness", textures.metallic_roughness),
        ("ao", textures.occlusion),
    ]
}

pub(crate) struct MaterialIds {
    materials: Vec<i64>,
}

impl MaterialIds {
    pub(crate) fn material(&self, id: MaterialId) -> Option<i64> {
        self.materials.get(id.index()).copied()
    }
}

pub(crate) fn write(
    model: &Model,
    composer: &mut Composer,
    ctx: &mut ExportCtx<'_>,
) -> Result<MaterialIds> {
    let mut videos = Vec::with_capacity(model.images.len());
    for index in 0..model.images.len() {
        ctx.report(
            Stage::Resources,
            index as f32 / model.images.len().max(1) as f32,
        );
        videos.push(video(model, index, composer, ctx)?);
    }

    let mut textures = Vec::with_capacity(model.textures.len());
    for (index, texture) in model.textures.iter().enumerate() {
        let id = composer.id();
        let name = match texture.name.is_empty() {
            true => format!("Texture{index}"),
            false => texture.name.clone(),
        };
        let file = file_name(model, texture.image.index());
        composer.add(
            "Texture",
            Node::new("Texture")
                .with_i64(id)
                .with_text(named(&name, "Texture"))
                .with_text("")
                .adding(Node::new("Type").with_text("TextureVideoClip"))
                .adding(Node::new("Version").with_i32(202))
                .adding(Node::new("TextureName").with_text(named(&name, "Texture")))
                .adding(
                    Node::new("Properties70")
                        .adding(prop("UseMaterial", "bool", "", "").with_i32(1))
                        .adding(prop("UVSet", "KString", "", "").with_text("UVMap"))
                        .adding(integer_wrap("WrapModeU", texture.sampler.wrap))
                        .adding(integer_wrap("WrapModeV", texture.sampler.wrap)),
                )
                .adding(Node::new("Media").with_text(named(&file, "Video")))
                .adding(Node::new("FileName").with_text(file.clone()))
                .adding(Node::new("RelativeFilename").with_text(file))
                .adding(Node::new("ModelUVTranslation").with_f64(0.0).with_f64(0.0))
                .adding(Node::new("ModelUVScaling").with_f64(1.0).with_f64(1.0))
                .adding(Node::new("Texture_Alpha_Source").with_text("None"))
                .adding(
                    Node::new("Cropping")
                        .with_i32(0)
                        .with_i32(0)
                        .with_i32(0)
                        .with_i32(0),
                ),
        );
        if let Some(video) = videos[texture.image.index()] {
            composer.connect(video, id);
        }
        textures.push(id);
    }

    let mut materials = Vec::with_capacity(model.materials.len());
    for model_material in &model.materials {
        let id = composer.id();
        composer.add("Material", material(model_material, id));
        for (property, slot) in slots(&model_material.textures) {
            if let Some(texture) = slot
                && let Some(&texture_id) = textures.get(texture.index())
            {
                composer.connect_property(texture_id, id, property);
            }
        }
        materials.push(id);
    }
    Ok(MaterialIds { materials })
}

fn material(material: &rikka_model_core::Material, id: i64) -> Node {
    let (metallic, roughness) = match material.shading {
        Shading::Pbr(pbr) => (pbr.metallic, pbr.roughness),
        Shading::Toon(toon) => (0.0, 1.0 - toon.shading_toony),
    };
    let color = material.base_color.truncate();
    let shininess = ((1.0 - roughness) / 0.1).powi(2);

    Node::new("Material")
        .with_i64(id)
        .with_text(named(&material.name, "Material"))
        .with_text("")
        .adding(Node::new("Version").with_i32(102))
        .adding(Node::new("ShadingModel").with_text("phong"))
        .adding(Node::new("MultiLayer").with_i32(0))
        .adding(
            Node::new("Properties70")
                .adding(vector("DiffuseColor", "Color", color))
                .adding(number("DiffuseFactor", 1.0))
                .adding(vector("AmbientColor", "Color", Vec3::ZERO))
                .adding(vector("EmissiveColor", "Color", material.emissive))
                .adding(number("EmissiveFactor", 1.0))
                .adding(vector("SpecularColor", "Color", Vec3::splat(0.2)))
                .adding(number("SpecularFactor", 1.0))
                .adding(number("Shininess", f64::from(shininess)))
                .adding(number("ShininessExponent", f64::from(shininess)))
                .adding(vector("TransparentColor", "Color", Vec3::ONE))
                .adding(number(
                    "TransparencyFactor",
                    f64::from(1.0 - material.base_color.w),
                ))
                .adding(number("Opacity", f64::from(material.base_color.w)))
                .adding(number("metalness", f64::from(metallic)))
                .adding(number("roughness", f64::from(roughness))),
        )
}

fn video(
    model: &Model,
    index: usize,
    composer: &mut Composer,
    ctx: &mut ExportCtx<'_>,
) -> Result<Option<i64>> {
    let image = &model.images[index];
    let file = file_name(model, index);
    let id = composer.id();
    let mut node = Node::new("Video")
        .with_i64(id)
        .with_text(named(&file, "Video"))
        .with_text("Clip")
        .adding(Node::new("Type").with_text("Clip"))
        .adding(
            Node::new("Properties70")
                .adding(prop("Path", "KString", "XRefUrl", "").with_text(file.clone())),
        )
        .adding(Node::new("UseMipMap").with_i32(0))
        .adding(Node::new("Filename").with_text(file.clone()))
        .adding(Node::new("RelativeFilename").with_text(file.clone()));

    match ctx.embeds() {
        true => node.add(Node::new("Content").with(Value::Raw(image.bytes.clone()))),
        false => {
            ctx.write_resource(&file, &image.bytes)?;
            &mut node
        }
    };
    composer.add("Video", node);
    Ok(Some(id))
}

fn file_name(model: &Model, index: usize) -> String {
    path::file_name(&model.images[index].file_name(index)).to_owned()
}

fn integer_wrap(name: &str, wrap: rikka_model_core::Wrap) -> Node {
    prop(name, "enum", "", "").with_i32(match wrap {
        rikka_model_core::Wrap::Repeat => 0,
        rikka_model_core::Wrap::Clamp => 1,
    })
}
