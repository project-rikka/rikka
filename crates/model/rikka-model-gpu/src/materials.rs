use rikka_model_core::glam::Vec3;
use rikka_renderer::TextureId as GpuTexture;
use rikka_renderer_3d::{
    BuiltinTemplate, Cull, FACE_LAYER, HAIR_LAYER, MaterialDesc, MaterialState, MaterialTemplate,
    NITS_PER_RADIANCE_UNIT, SEE_THROUGH_LAYER, TextureBinding, Value,
};
use rikka_shader_presets::import::{self, Class, SphereMode};

use rikka_model_core::{
    Alpha, MatcapBlend, Material, OutlineWidth, Pbr, Shading, Toon, ToonFlavor, ToonSurface,
};
use rikka_model_core::{Model, Sampler, TextureChannel, TextureId};

use crate::textures::{Encoding, Picture};

pub(crate) const MMD: &str = "mmd";
pub(crate) const MTOON: &str = "mtoon";

const SEE_THROUGH_AMOUNT: f32 = 0.35;
const SEE_THROUGH_REACH: f32 = 0.08;

type Described = rikka_renderer_3d::Result<MaterialDesc>;

const HAIR_CUTOFF: f32 = 0.5;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Binding {
    pub texture: Option<GpuTexture>,
    pub sampler: Sampler,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Bindings {
    pub albedo: Binding,
    pub normal: Binding,
    pub orm: Binding,
    pub emissive: Binding,
    pub shade: Binding,
    pub shading_shift: Binding,
    pub matcap: Binding,
    pub rim: Binding,
    pub outline_width: Binding,
    pub uv_animation_mask: Binding,
    pub shading_ramp: Binding,
    pub layers: LayerBindings,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LayerBindings {
    pub specular: Binding,
    pub specular_color: Binding,
    pub anisotropy: Binding,
    pub coat: Binding,
    pub coat_roughness: Binding,
    pub coat_normal: Binding,
    pub sheen_color: Binding,
    pub sheen_roughness: Binding,
    pub transmission: Binding,
    pub thickness: Binding,
    pub iridescence: Binding,
    pub iridescence_thickness: Binding,
}

impl Bindings {
    fn named(&self, shading: Shading) -> Vec<(&'static str, Binding)> {
        let layers = &self.layers;
        match shading {
            Shading::Pbr(_) => vec![
                ("base_color_texture", self.albedo),
                ("normal_texture", self.normal),
                ("metal_roughness_texture", self.orm),
                ("occlusion_texture", self.orm),
                ("emission_texture", self.emissive),
                ("specular_weight_texture", layers.specular),
                ("specular_color_texture", layers.specular_color),
                ("anisotropy_texture", layers.anisotropy),
                ("coat_texture", layers.coat),
                ("coat_roughness_texture", layers.coat_roughness),
                ("coat_normal_texture", layers.coat_normal),
                ("fuzz_color_texture", layers.sheen_color),
                ("fuzz_roughness_texture", layers.sheen_roughness),
                ("transmission_texture", layers.transmission),
                ("thickness_texture", layers.thickness),
                ("thin_film_texture", layers.iridescence),
                ("thin_film_thickness_texture", layers.iridescence_thickness),
            ],
            Shading::Toon(toon) => vec![
                (import::BASE_TEXTURE, self.albedo),
                (import::NORMAL_TEXTURE, self.normal),
                (import::EMISSION_TEXTURE, self.emissive),
                ("shade_texture", self.shade),
                ("shading_shift_texture", self.shading_shift),
                (
                    match toon.flavor {
                        ToonFlavor::Mmd => import::SPHERE_TEXTURE,
                        ToonFlavor::MToon => "matcap_texture",
                    },
                    self.matcap,
                ),
                ("rim_texture", self.rim),
                ("outline_width_texture", self.outline_width),
                ("uv_animation_mask_texture", self.uv_animation_mask),
                (import::TOON_TEXTURE, self.shading_ramp),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MaterialImage {
    Color(TextureId),
    Data(TextureId),
    Channel {
        texture: TextureId,
        channel: usize,
    },
    Surface {
        occlusion: Option<TextureId>,
        metallic_roughness: Option<TextureId>,
    },
}

pub(crate) fn bind(
    model: &Model,
    material: &Material,
    texture: &mut dyn FnMut(Picture) -> Option<GpuTexture>,
) -> Bindings {
    let described = |id: TextureId| model.textures.get(id.index());
    bind_with(material, &mut |request| match request {
        MaterialImage::Color(id) | MaterialImage::Data(id) => {
            let Some(found) = described(id) else {
                return Binding::default();
            };
            let encoding = match request {
                MaterialImage::Color(_) => Encoding::Srgb,
                _ => Encoding::Linear,
            };
            Binding {
                texture: texture(Picture::Plain {
                    image: found.image,
                    encoding,
                }),
                sampler: found.sampler,
            }
        }
        MaterialImage::Channel {
            texture: id,
            channel,
        } => {
            let Some(found) = described(id) else {
                return Binding::default();
            };
            Binding {
                texture: texture(Picture::Channel {
                    image: found.image,
                    channel,
                }),
                sampler: found.sampler,
            }
        }
        MaterialImage::Surface {
            occlusion,
            metallic_roughness,
        } => {
            let image_of = |id: Option<TextureId>| id.and_then(described).map(|found| found.image);
            Binding {
                texture: Picture::surface(image_of(occlusion), image_of(metallic_roughness))
                    .and_then(&mut *texture),
                sampler: metallic_roughness
                    .or(occlusion)
                    .and_then(described)
                    .map(|found| found.sampler)
                    .unwrap_or_default(),
            }
        }
    })
}

pub(crate) fn bind_with(
    material: &Material,
    resolve: &mut dyn FnMut(MaterialImage) -> Binding,
) -> Bindings {
    let textures = &material.textures;
    let channels = textures.channels;
    let mut request = |image: Option<MaterialImage>| image.map(&mut *resolve).unwrap_or_default();
    let color = |id: Option<TextureId>| id.map(MaterialImage::Color);
    let data = |id: Option<TextureId>| id.map(MaterialImage::Data);
    let scalar = |id: Option<TextureId>, channel: TextureChannel| {
        id.map(|texture| match channel.index() {
            Some(channel) => MaterialImage::Channel { texture, channel },
            None => MaterialImage::Data(texture),
        })
    };
    let albedo = request(color(textures.base_color));
    let emissive = request(color(textures.emissive));
    let shade = request(color(textures.shade));
    let matcap = request(color(textures.matcap));
    let rim = request(color(textures.rim));
    let shading_ramp = request(color(textures.shading_ramp));
    let specular_color = request(color(textures.specular_color));
    let sheen_color = request(color(textures.sheen_color));
    let normal = request(data(textures.normal));
    let shading_shift = request(data(textures.shading_shift));
    let outline_width = request(data(textures.outline_width));
    let uv_animation_mask = request(data(textures.uv_animation_mask));
    let anisotropy = request(data(textures.anisotropy));
    let coat_normal = request(data(textures.coat_normal));
    let layers = LayerBindings {
        specular: request(scalar(textures.specular, channels.specular)),
        specular_color,
        anisotropy,
        coat: request(scalar(textures.coat, channels.coat)),
        coat_roughness: request(scalar(textures.coat_roughness, channels.coat_roughness)),
        coat_normal,
        sheen_color,
        sheen_roughness: request(scalar(textures.sheen_roughness, channels.sheen_roughness)),
        transmission: request(scalar(textures.transmission, channels.transmission)),
        thickness: request(scalar(textures.thickness, channels.thickness)),
        iridescence: request(scalar(textures.iridescence, channels.iridescence)),
        iridescence_thickness: request(scalar(
            textures.iridescence_thickness,
            channels.iridescence_thickness,
        )),
    };
    let orm = match material.shading {
        Shading::Pbr(_) => request(textures.metallic_roughness.or(textures.occlusion).map(|_| {
            MaterialImage::Surface {
                occlusion: textures.occlusion,
                metallic_roughness: textures.metallic_roughness,
            }
        })),
        Shading::Toon(_) => Binding::default(),
    };

    Bindings {
        albedo,
        normal,
        orm,
        emissive,
        shade,
        shading_shift,
        matcap,
        rim,
        outline_width,
        uv_animation_mask,
        shading_ramp,
        layers,
    }
}

pub(crate) fn preset(material: &Material) -> Option<&'static str> {
    match material.shading {
        Shading::Pbr(_) => None,
        Shading::Toon(toon) => Some(match toon.flavor {
            ToonFlavor::Mmd => MMD,
            ToonFlavor::MToon => MTOON,
        }),
    }
}

pub(crate) fn template(material: &Material) -> MaterialTemplate {
    match preset(material) {
        Some(key) => rikka_shader_presets_gpu::template(key)
            .unwrap_or_else(|| panic!("the {key} shader preset is prebuilt")),
        None => MaterialTemplate::builtin(BuiltinTemplate::Standard),
    }
}

pub(crate) fn class(material: &Material) -> Option<ToonSurface> {
    match material.shading {
        Shading::Toon(toon) => Some(toon.surface),
        Shading::Pbr(_) => None,
    }
}

pub(crate) fn describe(
    material: &Material,
    bindings: &Bindings,
    filtering: impl Fn(Sampler) -> rikka_gpu::Filtering,
) -> Described {
    let template = template(material);
    let schema = template.schema();
    let mut desc = MaterialDesc::new(&template)
        .state(state(material))
        .alpha_cutoff(cutoff(material));
    if let Some(class) = class(material)
        && schema.presets().contains(class_preset(class).preset())
    {
        desc = desc.preset(class_preset(class).preset())?;
    }
    let known = |name: &str| schema.parameter(name).is_some();
    for (name, value) in values(material, bindings.orm.texture.is_some()) {
        if known(name) {
            desc.set(name, value)?;
        }
    }
    for (name, binding) in bindings.named(material.shading) {
        if !known(name) {
            continue;
        }
        let texture = binding
            .texture
            .map(|texture| TextureBinding::new(texture, filtering(binding.sampler)));
        desc.set(name, texture)?;
    }
    Ok(desc)
}

pub(crate) fn describe_outline(
    material: &Material,
    bindings: &Bindings,
    filtering: impl Fn(Sampler) -> rikka_gpu::Filtering,
) -> Described {
    Ok(describe(material, bindings, filtering)?
        .state(outline_state(material))
        .hull())
}

pub(crate) fn fallback() -> Described {
    describe(&Material::default(), &Bindings::default(), |_| {
        rikka_gpu::Filtering::LinearRepeat
    })
}

pub(crate) fn state(material: &Material) -> MaterialState {
    let state = match alpha(material) {
        Alpha::Opaque => MaterialState::opaque(),
        Alpha::Mask { .. } => MaterialState::masked(),
        Alpha::Blend => MaterialState::transparent(),
    };
    let state = match material.shading {
        Shading::Pbr(pbr) if pbr.transmission.weight > 0.0 => MaterialState::transparent(),
        _ => state,
    };
    let state = match (alpha(material), material.shading) {
        (Alpha::Blend, Shading::Toon(toon)) if toon.transparent_depth_write => {
            state.depth(true, true)
        }
        _ => state,
    };
    let state = match material.double_sided {
        true => state.double_sided(),
        false => state.cull(Cull::Back),
    };
    let solid = alpha(material) != Alpha::Blend || state.depth_write;
    state.shadows(material.casts_shadow && solid, material.receives_shadow)
}

fn alpha(material: &Material) -> Alpha {
    match (material.alpha, material.shading) {
        (Alpha::Blend, Shading::Toon(toon)) if toon.surface == ToonSurface::Hair => Alpha::Mask {
            cutoff: HAIR_CUTOFF,
        },
        (alpha, _) => alpha,
    }
}

pub(crate) fn outlined(material: &Material) -> bool {
    match material.shading {
        Shading::Toon(toon) => toon.outline.width.is_drawn(),
        Shading::Pbr(_) => false,
    }
}

pub(crate) fn outline_state(material: &Material) -> MaterialState {
    state(material).cull(Cull::Front).shadows(false, false)
}

pub fn class_preset(surface: ToonSurface) -> Class {
    match surface {
        ToonSurface::Cloth => Class::Cloth,
        ToonSurface::Skin => Class::Skin,
        ToonSurface::Face => Class::Face,
        ToonSurface::Hair => Class::Hair,
        ToonSurface::Eye => Class::Eye,
        ToonSurface::Mouth => Class::Mouth,
        ToonSurface::Lash => Class::Lash,
        ToonSurface::Blush => Class::Blush,
        ToonSurface::Stocking => Class::Stocking,
    }
}

pub(crate) fn character_layers(material: &Material) -> u32 {
    let Shading::Toon(toon) = material.shading else {
        return 0;
    };
    match toon.surface {
        ToonSurface::Hair => HAIR_LAYER,
        ToonSurface::Face | ToonSurface::Mouth | ToonSurface::Blush => FACE_LAYER,
        ToonSurface::Eye | ToonSurface::Lash => SEE_THROUGH_LAYER,
        ToonSurface::Cloth | ToonSurface::Skin | ToonSurface::Stocking => 0,
    }
}

pub(crate) fn values(material: &Material, has_orm: bool) -> Vec<(&'static str, Value)> {
    match material.shading {
        Shading::Pbr(pbr) => pbr_values(material, &pbr, has_orm),
        Shading::Toon(toon) => toon_values(material, &toon),
    }
}

const NANOMETRES_PER_MICROMETRE: f32 = 1000.0;

fn pbr_values(material: &Material, pbr: &Pbr, has_orm: bool) -> Vec<(&'static str, Value)> {
    let uv = material.uv_transform;
    let opacity = match material.alpha {
        Alpha::Opaque => 1.0,
        Alpha::Mask { .. } | Alpha::Blend => material.base_color.w,
    };
    let radius = pbr.subsurface.radius.max_element();
    let attenuation = pbr.transmission.attenuation_distance;
    vec![
        ("base_color", material.base_color.truncate().into()),
        ("geometry_opacity", opacity.into()),
        ("base_metalness", pbr.metallic.into()),
        ("base_diffuse_roughness", pbr.diffuse_roughness.into()),
        ("specular_roughness", pbr.roughness.into()),
        ("specular_weight", pbr.specular.weight.into()),
        ("specular_color", pbr.specular.color.into()),
        ("specular_ior", pbr.specular.ior.into()),
        (
            "specular_roughness_anisotropy",
            pbr.anisotropy.strength.into(),
        ),
        ("specular_rotation", pbr.anisotropy.rotation.into()),
        ("coat_weight", pbr.coat.weight.into()),
        ("coat_color", pbr.coat.color.into()),
        ("coat_roughness", pbr.coat.roughness.into()),
        ("coat_ior", pbr.coat.ior.into()),
        ("coat_darkening", pbr.coat.darkening.into()),
        ("coat_normal_scale", pbr.coat.normal_scale.into()),
        ("fuzz_weight", pbr.sheen.weight.into()),
        ("fuzz_color", pbr.sheen.color.into()),
        ("fuzz_roughness", pbr.sheen.roughness.into()),
        ("transmission_weight", pbr.transmission.weight.into()),
        (
            "transmission_color",
            pbr.transmission.attenuation_color.into(),
        ),
        (
            "transmission_depth",
            match attenuation.is_finite() {
                true => attenuation,
                false => 0.0,
            }
            .into(),
        ),
        (
            "transmission_dispersion_scale",
            pbr.transmission.dispersion.into(),
        ),
        ("geometry_thickness", pbr.transmission.thickness.into()),
        ("geometry_thin_walled", pbr.thin_walled.into()),
        ("subsurface_weight", pbr.subsurface.weight.into()),
        ("subsurface_color", pbr.subsurface.color.into()),
        ("subsurface_radius", radius.into()),
        (
            "subsurface_radius_scale",
            (pbr.subsurface.radius / radius.max(f32::MIN_POSITIVE)).into(),
        ),
        (
            "subsurface_scatter_anisotropy",
            pbr.subsurface.anisotropy.into(),
        ),
        ("thin_film_weight", pbr.iridescence.weight.into()),
        ("thin_film_ior", pbr.iridescence.ior.into()),
        (
            "thin_film_thickness",
            (pbr.iridescence.thickness_max / NANOMETRES_PER_MICROMETRE).into(),
        ),
        (
            "thin_film_thickness_minimum",
            (pbr.iridescence.thickness_min / NANOMETRES_PER_MICROMETRE).into(),
        ),
        ("emission_color", material.emissive.into()),
        ("emission_luminance", NITS_PER_RADIANCE_UNIT.into()),
        ("normal_scale", pbr.normal_scale.into()),
        (
            "occlusion_strength",
            match has_orm {
                true => pbr.occlusion_strength,
                false => 0.0,
            }
            .into(),
        ),
        ("uv.offset", uv.offset.into()),
        ("uv.scale", uv.scale.into()),
        ("uv.rotation", uv.rotation.into()),
    ]
}

fn toon_values(material: &Material, toon: &Toon) -> Vec<(&'static str, Value)> {
    let uv = material.uv_transform;
    let base = match toon.flavor {
        ToonFlavor::Mmd => (material.base_color.truncate() - toon.ambient)
            .max(Vec3::ZERO)
            .extend(material.base_color.w),
        ToonFlavor::MToon => material.base_color,
    };
    let sphere = match (toon.flavor, material.textures.matcap) {
        (ToonFlavor::Mmd, Some(_)) => match toon.matcap_blend {
            MatcapBlend::Add => SphereMode::Add,
            MatcapBlend::Multiply => SphereMode::Multiply,
        },
        _ => SphereMode::None,
    };
    let seen = matches!(toon.surface, ToonSurface::Eye | ToonSurface::Lash);
    vec![
        (import::BASE_COLOR, base.into()),
        (import::AMBIENT_COLOR, toon.ambient.into()),
        (import::SHADE_COLOR, toon.shade_color.into()),
        ("shading_shift", toon.shading_shift.into()),
        ("shading_shift_scale", toon.shading_shift_scale.into()),
        ("shading_toony", toon.shading_toony.into()),
        ("gi_equalization", toon.gi_equalization.into()),
        (import::NORMAL_SCALE, toon.normal_scale.into()),
        (import::EMISSION_COLOR, material.emissive.into()),
        ("matcap_color", toon.matcap_color.into()),
        (import::SPHERE_MODE, Value::Enum(sphere.name().into())),
        ("rim_color", toon.rim_color.into()),
        ("rim_fresnel_power", toon.rim_fresnel_power.into()),
        ("rim_lift", toon.rim_lift.into()),
        ("rim_lighting_mix", toon.rim_lighting_mix.into()),
        (import::SPECULAR_COLOR, toon.specular.into()),
        (import::SPECULAR_POWER, toon.specular_power.into()),
        (import::OUTLINE_COLOR, toon.outline.color.into()),
        ("outline_lighting_mix", toon.outline.lighting_mix.into()),
        (import::OUTLINE_WIDTH, toon.outline.width.width().into()),
        (
            "outline_width_mode",
            Value::Enum(
                match toon.outline.width {
                    OutlineWidth::Screen(_) => "screen",
                    OutlineWidth::World(_) | OutlineWidth::None => "world",
                }
                .into(),
            ),
        ),
        ("uv_offset", uv.offset.into()),
        ("uv_scale", uv.scale.into()),
        ("uv_rotation", uv.rotation.into()),
        ("uv_scroll", toon.scroll.into()),
        ("uv_spin", toon.spin.into()),
        (
            import::SEE_THROUGH_AMOUNT,
            match seen {
                true => SEE_THROUGH_AMOUNT,
                false => 0.0,
            }
            .into(),
        ),
        (import::SEE_THROUGH_REACH, SEE_THROUGH_REACH.into()),
    ]
}

pub(crate) fn cutoff(material: &Material) -> f32 {
    match alpha(material) {
        Alpha::Mask { cutoff } => cutoff,
        _ => 0.5,
    }
}
