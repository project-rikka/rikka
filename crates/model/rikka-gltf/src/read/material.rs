use std::collections::BTreeSet;
use std::sync::Arc;

use rikka_model_core::glam::{Vec3, Vec4};

use crate::document::{self, AlphaMode, CLAMP_TO_EDGE, Document, Index, MIRRORED_REPEAT, NEAREST};
use crate::read::buffers::Buffers;
use crate::uri::{self, Uri};
use rikka_model_core::ImportCtx;
use rikka_model_core::ModelBuilder;
use rikka_model_core::Result;
use rikka_model_core::Warning;
use rikka_model_core::mime;
use rikka_model_core::{
    Alpha, Anisotropy, Coat, Iridescence, Material, MaterialTextures, Pbr, Shading, Sheen,
    UvTransform,
};
use rikka_model_core::{Filter, Image, ImageId, Sampler, Texture, TextureId, Wrap};

const GLTF_COAT_IOR: f32 = 1.5;

#[derive(Debug, Default)]
pub struct TextureMap {
    pub(crate) textures: Vec<Option<TextureId>>,
    pub(crate) images: Vec<Option<ImageId>>,
}

impl TextureMap {
    #[must_use]
    pub fn texture(&self, index: u32) -> Option<TextureId> {
        self.textures.get(index as usize).copied().flatten()
    }

    #[must_use]
    pub fn image(&self, index: u32) -> Option<ImageId> {
        self.images.get(index as usize).copied().flatten()
    }
}

pub(crate) fn read(
    document: &Document,
    buffers: &Buffers<'_>,
    also_read: &[&str],
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<TextureMap> {
    let images = read_images(document, buffers, builder, ctx)?;
    let texture_map = TextureMap {
        textures: read_textures(document, &images, builder, ctx),
        images,
    };

    let mut unsupported = BTreeSet::new();
    for (index, described) in document.materials.iter().enumerate() {
        ctx.checkpoint()?;
        let shaded = described
            .extensions
            .other
            .keys()
            .any(|name| also_read.contains(&name.as_str()));
        unsupported.extend(
            described
                .extensions
                .other
                .keys()
                .filter(|name| !also_read.contains(&name.as_str()))
                .cloned(),
        );
        let material = read_material(described, index, &texture_map, shaded, ctx);
        builder.add_material(material);
    }
    for name in unsupported {
        ctx.warn(Warning::Unsupported(format!(
            "ignoring unsupported material extension \"{name}\""
        )));
    }
    Ok(texture_map)
}

fn read_images(
    document: &Document,
    buffers: &Buffers<'_>,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<Vec<Option<ImageId>>> {
    let mut images = Vec::with_capacity(document.images.len());
    for (index, described) in document.images.iter().enumerate() {
        ctx.checkpoint()?;
        images.push(read_image(
            document, buffers, described, index, builder, ctx,
        )?);
    }
    Ok(images)
}

fn read_image(
    document: &Document,
    buffers: &Buffers<'_>,
    described: &document::Image,
    index: usize,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<Option<ImageId>> {
    let image: Option<(String, Arc<[u8]>)> = match (&described.uri, described.buffer_view) {
        (Some(named), _) => match uri::parse(named)? {
            Uri::Data { mime, bytes } => {
                ctx.charge(bytes.len() as u64)?;
                Some((
                    named_image(described, index, mime.as_deref(), &bytes),
                    Arc::from(bytes),
                ))
            }
            Uri::File(path) => return ctx.import_image(builder, &[&path]),
            Uri::Remote(remote) => {
                ctx.warn(Warning::MissingResource {
                    uri: remote,
                    reason: "remote images are not fetched".to_owned(),
                });
                None
            }
        },
        (None, Some(view)) => {
            let bytes = buffers.view(document, view)?;
            ctx.charge(bytes.len() as u64)?;
            Some((
                named_image(described, index, described.mime_type.as_deref(), bytes),
                Arc::from(bytes),
            ))
        }
        (None, None) => {
            ctx.warn(Warning::Repaired(format!("skipping empty image {index}")));
            None
        }
    };
    Ok(image.map(|(name, bytes)| builder.add_image(Image::new(name, bytes))))
}

fn named_image(
    described: &document::Image,
    index: usize,
    mime: Option<&str>,
    bytes: &[u8],
) -> String {
    if let Some(name) = &described.name
        && !name.is_empty()
    {
        return name.clone();
    }
    let mime = mime.or_else(|| mime::sniff(bytes)).unwrap_or("image/png");
    format!("image{index}.{}", mime::extension(mime))
}

fn read_textures(
    document: &Document,
    images: &[Option<ImageId>],
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Vec<Option<TextureId>> {
    document
        .textures
        .iter()
        .enumerate()
        .map(|(index, described)| {
            let image = described
                .source
                .and_then(|source| images.get(source as usize).copied().flatten());
            let Some(image) = image else {
                ctx.warn(Warning::Repaired(format!(
                    "skipping texture {index} without a readable image"
                )));
                return None;
            };
            let texture = Texture::new(image)
                .named(described.name.clone().unwrap_or_default())
                .sampler(read_sampler(document, described.sampler, ctx));
            Some(builder.add_texture(texture))
        })
        .collect()
}

fn read_sampler(document: &Document, index: Option<Index>, ctx: &mut ImportCtx<'_>) -> Sampler {
    let Some(described) = index.and_then(|index| document.samplers.get(index as usize)) else {
        return Sampler::default();
    };
    let filter = match described.mag_filter {
        Some(NEAREST) => Filter::Nearest,
        _ => Filter::Linear,
    };
    let mut wrap = |mode: Option<u32>| match mode {
        Some(CLAMP_TO_EDGE) => Wrap::Clamp,
        Some(MIRRORED_REPEAT) => {
            ctx.warn(Warning::Unsupported(
                "mirrored repeat wrap is read as repeat".to_owned(),
            ));
            Wrap::Repeat
        }
        _ => Wrap::Repeat,
    };
    let across = wrap(described.wrap_s);
    if described.wrap_t.unwrap_or(0) != described.wrap_s.unwrap_or(0) {
        ctx.warn(Warning::Unsupported(
            "texture has different wrap modes on S and T, using one for both".to_owned(),
        ));
    }
    Sampler::new(filter, across)
}

fn read_material(
    described: &document::Material,
    index: usize,
    texture_map: &TextureMap,
    shaded: bool,
    ctx: &mut ImportCtx<'_>,
) -> Material {
    let pbr = described.pbr_metallic_roughness.clone().unwrap_or_default();
    let extensions = &described.extensions;
    let unlit = extensions.unlit.is_some();
    let strength = extensions
        .emissive_strength
        .as_ref()
        .map_or(1.0, |extension| extension.emissive_strength);
    let placement = pbr
        .base_color_texture
        .as_ref()
        .and_then(|info| info.extensions.texture_transform.as_ref())
        .map(uv_transform)
        .unwrap_or_default();

    let mut textures = MaterialTextures::default();
    let mut bind = |info: Option<&document::TextureInfo>| {
        let info = info?;
        report_texture(
            info.tex_coord,
            info.extensions.texture_transform.as_ref(),
            placement,
            ctx,
        );
        texture_map.texture(info.index)
    };
    textures.base_color = bind(pbr.base_color_texture.as_ref());
    textures.metallic_roughness = bind(pbr.metallic_roughness_texture.as_ref());
    textures.emissive = bind(described.emissive_texture.as_ref());
    let mut layers = Pbr {
        metallic: match unlit {
            true => 0.0,
            false => pbr.metallic_factor.unwrap_or(1.0),
        },
        roughness: match unlit {
            true => 1.0,
            false => pbr.roughness_factor.unwrap_or(1.0),
        },
        normal_scale: described
            .normal_texture
            .as_ref()
            .and_then(|normal| normal.scale)
            .unwrap_or(1.0),
        occlusion_strength: described
            .occlusion_texture
            .as_ref()
            .and_then(|occlusion| occlusion.strength)
            .unwrap_or(1.0),
        ..Pbr::default()
    };
    if let Some(ior) = extensions.ior.as_ref().and_then(|ior| ior.ior) {
        layers.specular.ior = ior;
    }
    if let Some(specular) = &extensions.specular {
        layers.specular.weight = specular.specular_factor.unwrap_or(1.0);
        layers.specular.color =
            Vec3::from_array(specular.specular_color_factor.unwrap_or([1.0; 3]));
        textures.specular = bind(specular.specular_texture.as_ref());
        textures.specular_color = bind(specular.specular_color_texture.as_ref());
    }
    if let Some(anisotropy) = &extensions.anisotropy {
        layers.anisotropy = Anisotropy {
            strength: anisotropy.anisotropy_strength.unwrap_or(0.0),
            rotation: anisotropy.anisotropy_rotation.unwrap_or(0.0),
        };
        textures.anisotropy = bind(anisotropy.anisotropy_texture.as_ref());
    }
    if let Some(clearcoat) = &extensions.clearcoat {
        layers.coat = Coat {
            weight: clearcoat.clearcoat_factor.unwrap_or(0.0),
            roughness: clearcoat.clearcoat_roughness_factor.unwrap_or(0.0),
            ior: GLTF_COAT_IOR,
            normal_scale: clearcoat
                .clearcoat_normal_texture
                .as_ref()
                .and_then(|normal| normal.scale)
                .unwrap_or(1.0),
            darkening: 0.0,
            ..Coat::default()
        };
        textures.coat = bind(clearcoat.clearcoat_texture.as_ref());
        textures.coat_roughness = bind(clearcoat.clearcoat_roughness_texture.as_ref());
    }
    if let Some(sheen) = &extensions.sheen {
        let color = Vec3::from_array(sheen.sheen_color_factor.unwrap_or([0.0; 3]));
        layers.sheen = Sheen {
            weight: match color.max_element() > 0.0 || sheen.sheen_color_texture.is_some() {
                true => 1.0,
                false => 0.0,
            },
            color,
            roughness: sheen.sheen_roughness_factor.unwrap_or(0.0),
        };
        textures.sheen_color = bind(sheen.sheen_color_texture.as_ref());
        textures.sheen_roughness = bind(sheen.sheen_roughness_texture.as_ref());
    }
    if let Some(transmission) = &extensions.transmission {
        layers.transmission.weight = transmission.transmission_factor.unwrap_or(0.0);
        textures.transmission = bind(transmission.transmission_texture.as_ref());
    }
    match &extensions.volume {
        Some(volume) => {
            layers.transmission.thickness = volume.thickness_factor.unwrap_or(0.0);
            layers.transmission.attenuation_color =
                Vec3::from_array(volume.attenuation_color.unwrap_or([1.0; 3]));
            layers.transmission.attenuation_distance =
                volume.attenuation_distance.unwrap_or(f32::INFINITY);
            layers.thin_walled = layers.transmission.thickness <= 0.0;
            textures.thickness = bind(volume.thickness_texture.as_ref());
        }
        None => layers.thin_walled = extensions.transmission.is_some(),
    }
    if let Some(dispersion) = extensions
        .dispersion
        .as_ref()
        .and_then(|dispersion| dispersion.dispersion)
    {
        layers.transmission.dispersion = dispersion;
    }
    if let Some(iridescence) = &extensions.iridescence {
        let defaults = Iridescence::default();
        layers.iridescence = Iridescence {
            weight: iridescence.iridescence_factor.unwrap_or(0.0),
            ior: iridescence.iridescence_ior.unwrap_or(defaults.ior),
            thickness_min: iridescence
                .iridescence_thickness_minimum
                .unwrap_or(defaults.thickness_min),
            thickness_max: iridescence
                .iridescence_thickness_maximum
                .unwrap_or(defaults.thickness_max),
        };
        textures.iridescence = bind(iridescence.iridescence_texture.as_ref());
        textures.iridescence_thickness = bind(iridescence.iridescence_thickness_texture.as_ref());
    }
    for (normal, slot) in [
        (described.normal_texture.as_ref(), &mut textures.normal),
        (
            extensions
                .clearcoat
                .as_ref()
                .and_then(|clearcoat| clearcoat.clearcoat_normal_texture.as_ref()),
            &mut textures.coat_normal,
        ),
    ] {
        if let Some(normal) = normal {
            report_texture(
                normal.tex_coord,
                normal.extensions.texture_transform.as_ref(),
                placement,
                ctx,
            );
            *slot = texture_map.texture(normal.index);
        }
    }
    if let Some(occlusion) = &described.occlusion_texture {
        report_texture(
            occlusion.tex_coord,
            occlusion.extensions.texture_transform.as_ref(),
            placement,
            ctx,
        );
        textures.occlusion = texture_map.texture(occlusion.index);
    }

    let name = described
        .name
        .clone()
        .unwrap_or_else(|| format!("material{index}"));
    if unlit && !shaded {
        ctx.warn(Warning::Unsupported(format!(
            "material \"{name}\": unlit is read as a rough dielectric"
        )));
    }

    Material {
        name,
        base_color: Vec4::from_array(pbr.base_color_factor.unwrap_or([1.0; 4])),
        emissive: Vec3::from_array(described.emissive_factor.unwrap_or([0.0; 3])) * strength,
        alpha: match described.alpha_mode {
            AlphaMode::Opaque => Alpha::Opaque,
            AlphaMode::Mask => Alpha::Mask {
                cutoff: described.alpha_cutoff.unwrap_or(0.5),
            },
            AlphaMode::Blend => Alpha::Blend,
        },
        double_sided: described.double_sided,
        casts_shadow: true,
        receives_shadow: true,
        shading: Shading::Pbr(layers),
        textures,
        uv_transform: placement,
    }
}

fn uv_transform(transform: &document::TextureTransform) -> UvTransform {
    UvTransform {
        offset: rikka_model_core::glam::Vec2::from_array(transform.offset.unwrap_or([0.0; 2])),
        scale: rikka_model_core::glam::Vec2::from_array(transform.scale.unwrap_or([1.0; 2])),
        rotation: transform.rotation.unwrap_or(0.0),
    }
}
fn report_texture(
    tex_coord: u32,
    transform: Option<&document::TextureTransform>,
    placement: UvTransform,
    ctx: &mut ImportCtx<'_>,
) {
    if tex_coord != 0 {
        ctx.warn(Warning::Unsupported(format!(
            "material uses TEXCOORD_{tex_coord}, only TEXCOORD_0 is supported"
        )));
    }
    let texture_transform = transform.map(uv_transform).unwrap_or_default();
    if texture_transform != placement {
        ctx.warn(Warning::Unsupported(
            "KHR_texture_transform differs between textures of one material, using the base colour transform"
                .to_owned(),
        ));
    }
}
