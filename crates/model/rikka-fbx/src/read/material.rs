use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rikka_model_core::glam::{Vec2, Vec3};
use rustc_hash::FxHashMap;

use crate::document::Node;
use crate::read::graph::{Graph, Object};
use crate::read::props::Props;
use rikka_model_core::ImportCtx;
use rikka_model_core::ModelBuilder;
use rikka_model_core::Result;
use rikka_model_core::Warning;
use rikka_model_core::path;
use rikka_model_core::{
    Alpha, Material, MaterialId, MaterialTextures, Pbr, Shading, TextureChannel, UvTransform,
};
use rikka_model_core::{Filter, Image, ImageId, Sampler, Texture, TextureId, Wrap};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    BaseColor,
    Normal,
    Emissive,
    Roughness,
    Metallic,
    Occlusion,
    Specular,
    SpecularColor,
    Coat,
    CoatRoughness,
    CoatNormal,
    SheenColor,
    SheenRoughness,
    Transmission,
}

const SLOTS: &[(&str, Slot)] = &[
    ("DiffuseColor", Slot::BaseColor),
    ("Diffuse", Slot::BaseColor),
    ("Maya|baseColor", Slot::BaseColor),
    ("Maya|TEX_color_map", Slot::BaseColor),
    ("base_color_map", Slot::BaseColor),
    ("baseColor", Slot::BaseColor),
    ("base_color", Slot::BaseColor),
    ("NormalMap", Slot::Normal),
    ("Bump", Slot::Normal),
    ("Maya|normalCamera", Slot::Normal),
    ("Maya|TEX_normal_map", Slot::Normal),
    ("normalCamera", Slot::Normal),
    ("norm_map", Slot::Normal),
    ("bump_map", Slot::Normal),
    ("norm", Slot::Normal),
    ("EmissiveColor", Slot::Emissive),
    ("Emissive", Slot::Emissive),
    ("Maya|emissive", Slot::Emissive),
    ("Maya|emissionColor", Slot::Emissive),
    ("Maya|TEX_emissive_map", Slot::Emissive),
    ("emit_color_map", Slot::Emissive),
    ("emission_map", Slot::Emissive),
    ("Maya|roughness", Slot::Roughness),
    ("Maya|specularRoughness", Slot::Roughness),
    ("Maya|TEX_roughness_map", Slot::Roughness),
    ("roughness_map", Slot::Roughness),
    ("roughness", Slot::Roughness),
    ("Maya|metallic", Slot::Metallic),
    ("Maya|metalness", Slot::Metallic),
    ("Maya|TEX_metallic_map", Slot::Metallic),
    ("metalness_map", Slot::Metallic),
    ("metalness", Slot::Metallic),
    ("Maya|ao", Slot::Occlusion),
    ("Maya|TEX_ao_map", Slot::Occlusion),
    ("ao_map", Slot::Occlusion),
    ("ao", Slot::Occlusion),
    ("Maya|specular", Slot::Specular),
    ("reflectivity_map", Slot::Specular),
    ("Maya|specularColor", Slot::SpecularColor),
    ("refl_color_map", Slot::SpecularColor),
    ("Maya|coat", Slot::Coat),
    ("coat_map", Slot::Coat),
    ("Maya|coatRoughness", Slot::CoatRoughness),
    ("coat_rough_map", Slot::CoatRoughness),
    ("Maya|coatNormal", Slot::CoatNormal),
    ("coat_bump_map", Slot::CoatNormal),
    ("Maya|sheenColor", Slot::SheenColor),
    ("sheen_color_map", Slot::SheenColor),
    ("Maya|sheenRoughness", Slot::SheenRoughness),
    ("sheen_rough_map", Slot::SheenRoughness),
    ("Maya|transmission", Slot::Transmission),
    ("transparency_map", Slot::Transmission),
];

#[derive(Default)]
pub(crate) struct MaterialTable {
    pub materials: FxHashMap<i64, MaterialId>,
    textures: FxHashMap<i64, TextureId>,
    images: FxHashMap<i64, ImageId>,
    named: FxHashMap<String, ImageId>,
}

pub(crate) fn read(
    graph: &Graph<'_>,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<MaterialTable> {
    let mut material_table = MaterialTable::default();
    for object in graph.objects().filter(|object| object.kind == "Video") {
        ctx.checkpoint()?;
        if let Some(image) = material_table.image(object, builder, ctx)? {
            material_table.images.insert(object.id, image);
        }
    }
    for object in graph.objects().filter(|object| object.kind == "Texture") {
        ctx.checkpoint()?;
        if let Some(texture) = material_table.texture(object, graph, builder, ctx)? {
            material_table.textures.insert(object.id, texture);
        }
    }
    for object in graph.objects().filter(|object| object.kind == "Material") {
        ctx.checkpoint()?;
        let material = material_table.material(object, graph, ctx);
        material_table
            .materials
            .insert(object.id, builder.add_material(material));
    }
    Ok(material_table)
}

impl MaterialTable {
    fn image(
        &mut self,
        object: &Object<'_>,
        builder: &mut ModelBuilder,
        ctx: &mut ImportCtx<'_>,
    ) -> Result<Option<ImageId>> {
        let embedded = object.node.child("Content").and_then(embedded);
        if let Some(bytes) = embedded {
            ctx.charge(bytes.len() as u64)?;
            let name = file_name(object).unwrap_or_else(|| object.name.clone());
            return Ok(Some(builder.add_image(Image::new(name, bytes))));
        }
        let Some(uri) = file_name(object) else {
            return Ok(None);
        };
        self.beside(&uri, builder, ctx)
    }

    fn beside(
        &mut self,
        uri: &str,
        builder: &mut ModelBuilder,
        ctx: &mut ImportCtx<'_>,
    ) -> Result<Option<ImageId>> {
        let plain = path::file_name(uri).to_owned();
        if let Some(&image) = self.named.get(&plain) {
            return Ok(Some(image));
        }
        let Some(image) = ctx.import_image(builder, &[uri, &plain])? else {
            return Ok(None);
        };
        self.named.insert(plain, image);
        Ok(Some(image))
    }

    fn texture(
        &mut self,
        object: &Object<'_>,
        graph: &Graph<'_>,
        builder: &mut ModelBuilder,
        ctx: &mut ImportCtx<'_>,
    ) -> Result<Option<TextureId>> {
        let linked = graph
            .sources(object.id)
            .find_map(|(source, _)| self.images.get(&source.id).copied());
        let image = match linked {
            Some(image) => image,
            None => {
                let Some(uri) = file_name(object) else {
                    return Ok(None);
                };
                let Some(image) = self.beside(&uri, builder, ctx)? else {
                    return Ok(None);
                };
                image
            }
        };

        let wrap = |name: &str| match object.props.integer(name) {
            Some(1) => Wrap::Clamp,
            _ => Wrap::Repeat,
        };
        Ok(Some(
            builder.add_texture(
                Texture::new(image)
                    .named(object.name.clone())
                    .sampler(Sampler::new(Filter::Linear, wrap("WrapModeU"))),
            ),
        ))
    }

    fn material(
        &self,
        object: &Object<'_>,
        graph: &Graph<'_>,
        ctx: &mut ImportCtx<'_>,
    ) -> Material {
        let props = &object.props;
        let factor = |name: &str| props.real(name, 1.0);
        let diffuse = props.vec3("DiffuseColor", Vec3::splat(0.8)) * factor("DiffuseFactor");
        let emissive = props.vec3("EmissiveColor", Vec3::ZERO) * factor("EmissiveFactor");

        let opacity = props
            .number("Opacity")
            .map(|value| value as f32)
            .unwrap_or_else(|| 1.0 - transparency(props));
        let base = ["Maya|base_color", "base_color", "baseColor"]
            .into_iter()
            .find(|name| props.get(name).is_some())
            .map_or(diffuse, |name| props.vec3(name, diffuse));

        let metallic = props
            .number("Maya|metallic")
            .or_else(|| props.number("Maya|metalness"))
            .or_else(|| props.number("metalness"))
            .or_else(|| props.number("ReflectionFactor"))
            .unwrap_or(0.0) as f32;
        let roughness = props
            .number("Maya|roughness")
            .or_else(|| props.number("Maya|specularRoughness"))
            .or_else(|| props.number("roughness"))
            .map(|value| value as f32)
            .unwrap_or_else(|| shininess(props));

        let mut textures = MaterialTextures::default();
        let mut uv_transform = UvTransform::default();
        let mut roughness_texture = None;
        for (source, property) in graph.sources(object.id) {
            let Some(property) = property else {
                continue;
            };
            let Some(&texture) = self.textures.get(&source.id) else {
                continue;
            };
            let Some(slot) = SLOTS
                .iter()
                .find(|(name, _)| *name == property)
                .map(|(_, slot)| *slot)
            else {
                continue;
            };
            let scalar = TextureChannel::Red;
            match slot {
                Slot::BaseColor => {
                    textures.base_color = Some(texture);
                    uv_transform = coordinates(source, ctx);
                }
                Slot::Normal => textures.normal = Some(texture),
                Slot::Emissive => textures.emissive = Some(texture),
                Slot::Occlusion => textures.occlusion = Some(texture),
                Slot::Roughness => roughness_texture = Some(texture),
                Slot::Metallic => textures.metallic_roughness = Some(texture),
                Slot::Specular => {
                    textures.specular = Some(texture);
                    textures.channels.specular = scalar;
                }
                Slot::SpecularColor => textures.specular_color = Some(texture),
                Slot::Coat => {
                    textures.coat = Some(texture);
                    textures.channels.coat = scalar;
                }
                Slot::CoatRoughness => {
                    textures.coat_roughness = Some(texture);
                    textures.channels.coat_roughness = scalar;
                }
                Slot::CoatNormal => textures.coat_normal = Some(texture),
                Slot::SheenColor => textures.sheen_color = Some(texture),
                Slot::SheenRoughness => {
                    textures.sheen_roughness = Some(texture);
                    textures.channels.sheen_roughness = scalar;
                }
                Slot::Transmission => {
                    textures.transmission = Some(texture);
                    textures.channels.transmission = scalar;
                }
            }
        }
        if let Some(texture) = roughness_texture {
            if textures
                .metallic_roughness
                .is_some_and(|current| current != texture)
            {
                ctx.warn(Warning::Unsupported(format!(
                    "material \"{}\": separate roughness and metalness textures, reading only one",
                    object.name
                )));
            }
            textures.metallic_roughness = Some(texture);
        }

        let mut pbr = layers(props);
        pbr.metallic = metallic.clamp(0.0, 1.0);
        pbr.roughness = roughness.clamp(0.0, 1.0);
        pbr.normal_scale = props.real("BumpFactor", 1.0);
        let base_weight = first_number(props, BASE_WEIGHT).unwrap_or(1.0);
        Material {
            name: object.name.clone(),
            base_color: (base * base_weight).extend(opacity.clamp(0.0, 1.0)),
            emissive,
            alpha: match opacity < 1.0 || pbr.transmission.weight > 0.0 {
                true => Alpha::Blend,
                false => Alpha::Opaque,
            },
            double_sided: pbr.thin_walled,
            casts_shadow: true,
            receives_shadow: true,
            shading: Shading::Pbr(pbr),
            textures,
            uv_transform,
        }
    }
}

const BASE_WEIGHT: &[&str] = &["Maya|base", "base_weight"];
const DIFFUSE_ROUGHNESS: &[&str] = &["Maya|diffuseRoughness", "diff_roughness"];
const SPECULAR_WEIGHT: &[&str] = &["Maya|specular", "reflectivity"];
const SPECULAR_COLOR: &[&str] = &["Maya|specularColor", "refl_color"];
const SPECULAR_IOR: &[&str] = &["Maya|specularIOR", "trans_ior"];
const ANISOTROPY: &[&str] = &["Maya|specularAnisotropy"];
const ANISOTROPY_ROTATION: &[&str] = &["Maya|specularRotation"];
const COAT: &[&str] = &["Maya|coat", "coating"];
const COAT_COLOR: &[&str] = &["Maya|coatColor", "coat_color"];
const COAT_ROUGHNESS: &[&str] = &["Maya|coatRoughness", "coat_roughness"];
const COAT_IOR: &[&str] = &["Maya|coatIOR", "coat_ior"];
const SHEEN: &[&str] = &["Maya|sheen", "sheen"];
const SHEEN_COLOR: &[&str] = &["Maya|sheenColor", "sheen_color"];
const SHEEN_ROUGHNESS: &[&str] = &["Maya|sheenRoughness", "sheen_roughness"];
const TRANSMISSION: &[&str] = &["Maya|transmission", "transparency"];
const TRANSMISSION_COLOR: &[&str] = &["Maya|transmissionColor", "trans_color"];
const TRANSMISSION_DEPTH: &[&str] = &["Maya|transmissionDepth", "trans_depth"];
const SUBSURFACE: &[&str] = &["Maya|subsurface", "scattering"];
const SUBSURFACE_COLOR: &[&str] = &["Maya|subsurfaceColor", "sss_color"];
const SUBSURFACE_RADIUS: &[&str] = &["Maya|subsurfaceRadius", "sss_scatter_color"];
const SUBSURFACE_SCALE: &[&str] = &["Maya|subsurfaceScale", "sss_depth"];
const THIN_FILM_THICKNESS: &[&str] = &["Maya|thinFilmThickness"];
const THIN_FILM_IOR: &[&str] = &["Maya|thinFilmIOR"];
const THIN_WALLED: &[&str] = &["Maya|thinWalled", "thin_walled"];

fn first_number(props: &Props, names: &[&str]) -> Option<f32> {
    names
        .iter()
        .find_map(|name| props.number(name))
        .map(|value| value as f32)
}

fn first_color(props: &Props, names: &[&str], fallback: Vec3) -> Vec3 {
    names
        .iter()
        .find(|name| props.get(name).is_some())
        .map_or(fallback, |name| props.vec3(name, fallback))
}

fn layers(props: &Props) -> Pbr {
    let defaults = Pbr::default();
    let number = |names, fallback| first_number(props, names).unwrap_or(fallback);
    let mut pbr = Pbr {
        diffuse_roughness: number(DIFFUSE_ROUGHNESS, 0.0),
        thin_walled: number(THIN_WALLED, 0.0) > 0.5,
        ..defaults
    };
    pbr.specular.weight = number(SPECULAR_WEIGHT, defaults.specular.weight);
    pbr.specular.color = first_color(props, SPECULAR_COLOR, defaults.specular.color);
    pbr.specular.ior = number(SPECULAR_IOR, defaults.specular.ior);
    pbr.anisotropy.strength = number(ANISOTROPY, 0.0);
    pbr.anisotropy.rotation = number(ANISOTROPY_ROTATION, 0.0) * std::f32::consts::TAU;
    pbr.coat.weight = number(COAT, 0.0);
    pbr.coat.color = first_color(props, COAT_COLOR, defaults.coat.color);
    pbr.coat.roughness = number(COAT_ROUGHNESS, defaults.coat.roughness);
    pbr.coat.ior = number(COAT_IOR, defaults.coat.ior);
    pbr.sheen.weight = number(SHEEN, 0.0);
    pbr.sheen.color = first_color(props, SHEEN_COLOR, defaults.sheen.color);
    pbr.sheen.roughness = number(SHEEN_ROUGHNESS, defaults.sheen.roughness);
    pbr.transmission.weight = number(TRANSMISSION, 0.0);
    pbr.transmission.attenuation_color = first_color(
        props,
        TRANSMISSION_COLOR,
        defaults.transmission.attenuation_color,
    );
    let depth = number(TRANSMISSION_DEPTH, 0.0);
    pbr.transmission.attenuation_distance = match depth > 0.0 {
        true => depth,
        false => f32::INFINITY,
    };
    pbr.subsurface.weight = number(SUBSURFACE, 0.0);
    pbr.subsurface.color = first_color(props, SUBSURFACE_COLOR, defaults.subsurface.color);
    pbr.subsurface.radius = first_color(props, SUBSURFACE_RADIUS, defaults.subsurface.radius)
        * number(SUBSURFACE_SCALE, 1.0);
    if let Some(thickness) = first_number(props, THIN_FILM_THICKNESS).filter(|nm| *nm > 0.0) {
        pbr.iridescence.weight = 1.0;
        pbr.iridescence.thickness_max = thickness;
        pbr.iridescence.ior = number(THIN_FILM_IOR, defaults.iridescence.ior);
    }
    pbr
}

fn coordinates(texture: &Object<'_>, ctx: &mut ImportCtx<'_>) -> UvTransform {
    let translation = texture.props.vec3("Translation", Vec3::ZERO);
    let scaling = texture.props.vec3("Scaling", Vec3::ONE);
    let rotation = texture.props.vec3("Rotation", Vec3::ZERO);
    if rotation != Vec3::ZERO {
        ctx.warn(Warning::Unsupported(format!(
            "texture \"{}\": UV rotation is not supported",
            texture.name
        )));
    }
    UvTransform {
        offset: Vec2::new(translation.x, 1.0 - scaling.y - translation.y),
        scale: Vec2::new(scaling.x, scaling.y),
        rotation: 0.0,
    }
}

fn transparency(props: &Props) -> f32 {
    if let Some(factor) = props.number("TransparencyFactor") {
        return factor as f32;
    }
    match props.get("TransparentColor") {
        Some(_) => {
            let color = props.vec3("TransparentColor", Vec3::ZERO);
            (color.x + color.y + color.z) / 3.0
        }
        None => 0.0,
    }
}

fn shininess(props: &Props) -> f32 {
    let shininess = props
        .number("Shininess")
        .or_else(|| props.number("ShininessExponent"))
        .unwrap_or(0.0) as f32;
    match shininess >= 0.0 {
        true => (1.0 - shininess.sqrt() * 0.1).clamp(0.0, 1.0),
        false => 1.0,
    }
}

fn embedded(node: &Node) -> Option<Vec<u8>> {
    if let Some(bytes) = node.raw(0).filter(|bytes| !bytes.is_empty()) {
        return Some(bytes.to_vec());
    }
    let text = node.text(0).filter(|text| !text.is_empty())?;
    STANDARD.decode(text).ok().filter(|bytes| !bytes.is_empty())
}
fn file_name(object: &Object<'_>) -> Option<String> {
    let path_property = |name: &str| {
        object
            .node
            .child_text(name)
            .or_else(|| object.props.text(name))
            .map(str::to_owned)
            .filter(|text| !text.is_empty())
    };
    path_property("RelativeFilename")
        .or_else(|| path_property("Filename"))
        .or_else(|| path_property("FileName"))
        .or_else(|| path_property("Path"))
}
