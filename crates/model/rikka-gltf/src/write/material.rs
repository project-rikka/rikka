use crate::document::{
    AlphaMode, CLAMP_TO_EDGE, Document, Image as DescribedImage, Index, LINEAR,
    LINEAR_MIPMAP_LINEAR, Material as DescribedMaterial, NEAREST, NormalTextureInfo,
    OcclusionTextureInfo, PbrMetallicRoughness, REPEAT, Sampler as DescribedSampler,
    Texture as DescribedTexture, TextureInfo, TextureInfoExtensions, TextureTransform,
};
use crate::uri;
use crate::write::buffer::Buffered;
use rikka_model_core::ExportCtx;
use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::Warning;
use rikka_model_core::mime;
use rikka_model_core::{Alpha, Material, Shading, UvTransform};
use rikka_model_core::{Filter, Sampler, TextureId, Wrap};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Images {
    Buffer,
    Data,
    Beside,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toon {
    Flattened,
    Kept,
}
pub(crate) fn write(
    model: &Model,
    document: &mut Document,
    buffered: &mut Buffered,
    ctx: &mut ExportCtx<'_>,
    images: Images,
    toon: Toon,
) -> Result<()> {
    document.images = write_images(model, buffered, ctx, images)?;
    write_textures(model, document);
    document.materials = model
        .materials
        .iter()
        .map(|material| described(material, ctx, toon))
        .collect();
    if document
        .materials
        .iter()
        .any(|material| moved(material.pbr_metallic_roughness.as_ref()))
    {
        document.extensions_used.push(TEXTURE_TRANSFORM.to_owned());
    }
    Ok(())
}

const TEXTURE_TRANSFORM: &str = "KHR_texture_transform";

fn moved(pbr: Option<&PbrMetallicRoughness>) -> bool {
    pbr.and_then(|pbr| pbr.base_color_texture.as_ref())
        .is_some_and(|info| info.extensions.texture_transform.is_some())
}

fn write_images(
    model: &Model,
    buffered: &mut Buffered,
    ctx: &mut ExportCtx<'_>,
    images: Images,
) -> Result<Vec<DescribedImage>> {
    let mut written = Vec::with_capacity(model.images.len());
    for (index, image) in model.images.iter().enumerate() {
        ctx.checkpoint()?;
        let mime = mime::guess(&image.bytes, &image.name);
        let name = image.file_name(index).into_owned();
        written.push(match images {
            Images::Buffer => DescribedImage {
                buffer_view: Some(buffered.raw(&image.bytes)),
                mime_type: Some(mime.to_owned()),
                name: Some(name),
                uri: None,
            },
            Images::Data => DescribedImage {
                uri: Some(uri::embed(mime, &image.bytes)),
                mime_type: Some(mime.to_owned()),
                name: Some(name),
                buffer_view: None,
            },
            Images::Beside => {
                ctx.write_resource(&name, &image.bytes)?;
                DescribedImage {
                    uri: Some(uri::relative(&name)),
                    mime_type: Some(mime.to_owned()),
                    name: Some(name),
                    buffer_view: None,
                }
            }
        });
    }
    Ok(written)
}

fn write_textures(model: &Model, document: &mut Document) {
    let mut samplers: Vec<Sampler> = Vec::new();
    document.textures = model
        .textures
        .iter()
        .map(|texture| {
            let sampler = samplers
                .iter()
                .position(|sampler| *sampler == texture.sampler)
                .unwrap_or_else(|| {
                    samplers.push(texture.sampler);
                    samplers.len() - 1
                });
            DescribedTexture {
                source: Some(texture.image.0),
                sampler: Some(sampler as Index),
                name: Some(texture.name.clone()).filter(|name| !name.is_empty()),
            }
        })
        .collect();

    document.samplers = samplers
        .into_iter()
        .map(|sampler| {
            let wrap = match sampler.wrap {
                Wrap::Repeat => REPEAT,
                Wrap::Clamp => CLAMP_TO_EDGE,
            };
            let (mag, min) = match sampler.filter {
                Filter::Nearest => (NEAREST, NEAREST),
                Filter::Linear => (LINEAR, LINEAR_MIPMAP_LINEAR),
            };
            DescribedSampler {
                mag_filter: Some(mag),
                min_filter: Some(min),
                wrap_s: Some(wrap),
                wrap_t: Some(wrap),
                name: None,
            }
        })
        .collect();
}

fn described(material: &Material, ctx: &mut ExportCtx<'_>, toon: Toon) -> DescribedMaterial {
    let pbr = match (material.shading, toon) {
        (Shading::Pbr(pbr), _) => pbr,
        (Shading::Toon(toon), Toon::Kept) => rikka_model_core::Pbr {
            metallic: 0.0,
            roughness: 1.0,
            normal_scale: toon.normal_scale,
            occlusion_strength: 1.0,
            ..rikka_model_core::Pbr::default()
        },
        (Shading::Toon(_), Toon::Flattened) => {
            ctx.warn(Warning::Unsupported(format!(
                "material \"{}\": toon shading exported as metallic-roughness",
                material.name
            )));
            rikka_model_core::Pbr {
                metallic: 0.0,
                roughness: 1.0,
                ..rikka_model_core::Pbr::default()
            }
        }
    };
    if toon == Toon::Flattened && (!material.casts_shadow || !material.receives_shadow) {
        ctx.warn(Warning::Unsupported(format!(
            "material \"{}\": shadow casting and receiving flags are not exported",
            material.name
        )));
    }

    let placement = transform_extensions(material.uv_transform);
    let strength = material.emissive.max_element().max(1.0);
    let textures = &material.textures;
    DescribedMaterial {
        pbr_metallic_roughness: Some(PbrMetallicRoughness {
            base_color_factor: Some(material.base_color.to_array()),
            base_color_texture: info(textures.base_color, &placement),
            metallic_factor: Some(pbr.metallic),
            roughness_factor: Some(pbr.roughness),
            metallic_roughness_texture: info(textures.metallic_roughness, &placement),
        }),
        normal_texture: info(textures.normal, &placement).map(|texture_info| NormalTextureInfo {
            index: texture_info.index,
            tex_coord: 0,
            scale: Some(pbr.normal_scale),
            extensions: texture_info.extensions,
        }),
        occlusion_texture: info(textures.occlusion, &placement).map(|texture_info| {
            OcclusionTextureInfo {
                index: texture_info.index,
                tex_coord: 0,
                strength: Some(pbr.occlusion_strength),
                extensions: texture_info.extensions,
            }
        }),
        emissive_texture: info(textures.emissive, &placement),
        emissive_factor: Some((material.emissive / strength).to_array()),
        alpha_mode: match material.alpha {
            Alpha::Opaque => AlphaMode::Opaque,
            Alpha::Mask { .. } => AlphaMode::Mask,
            Alpha::Blend => AlphaMode::Blend,
        },
        alpha_cutoff: match material.alpha {
            Alpha::Mask { cutoff } => Some(cutoff),
            _ => None,
        },
        double_sided: material.double_sided,
        name: Some(material.name.clone()).filter(|name| !name.is_empty()),
        extensions: {
            let mut extensions = emissive_strength(strength);
            crate::write::khr::write(&mut extensions, &pbr, textures, &placement);
            extensions
        },
    }
}

fn emissive_strength(strength: f32) -> crate::document::MaterialExtensions {
    let mut extensions = crate::document::MaterialExtensions::default();
    if strength > 1.0 {
        extensions.emissive_strength = Some(crate::document::EmissiveStrength {
            emissive_strength: strength,
        });
    }
    extensions
}

fn transform_extensions(transform: UvTransform) -> TextureInfoExtensions {
    let mut extensions = TextureInfoExtensions::default();
    if !transform.is_identity() {
        extensions.texture_transform = Some(TextureTransform {
            offset: Some(transform.offset.to_array()),
            rotation: Some(transform.rotation),
            scale: Some(transform.scale.to_array()),
            tex_coord: None,
        });
    }
    extensions
}

fn info(texture: Option<TextureId>, placement: &TextureInfoExtensions) -> Option<TextureInfo> {
    texture.map(|texture| TextureInfo {
        index: texture.0,
        tex_coord: 0,
        extensions: placement.clone(),
    })
}
