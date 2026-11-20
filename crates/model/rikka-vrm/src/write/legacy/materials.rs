use std::collections::BTreeMap;
use std::f32::consts::TAU;

use rikka_model_core::glam::{Vec3, Vec4};
use serde_json::Value;

use rikka_model_core::ExportCtx;
use rikka_model_core::Warning;
use rikka_model_core::{Alpha, Material, Model, OutlineWidth, Shading, TextureId, Toon};

use super::super::super::legacy::{self as raw, gamma};

const MTOON: &str = "VRM/MToon";

const GLTF_SHADER: &str = "VRM_USE_GLTFSHADER";

const MTOON_VERSION: f32 = 39.0;

const GEOMETRY_QUEUE: i32 = 2000;

const ALPHA_TEST_QUEUE: i32 = 2450;

const TRANSPARENT_QUEUE: i32 = 3000;

const TRANSPARENT_DEPTH_QUEUE: i32 = 2501;

const BLEND_ONE: f32 = 1.0;

const BLEND_ZERO: f32 = 0.0;

const BLEND_SOURCE_ALPHA: f32 = 5.0;

const BLEND_ONE_MINUS_SOURCE_ALPHA: f32 = 10.0;

const CULL_OFF: f32 = 0.0;

const CULL_FRONT: f32 = 1.0;

const CULL_BACK: f32 = 2.0;

const DEVIATION: f32 = 1e-4;

#[derive(Clone, Copy)]
enum RenderMode {
    Opaque,
    Cutout,
    Transparent,
    TransparentWithZWrite,
}

impl RenderMode {
    fn of(material: &Material, toon: &Toon) -> Self {
        match (material.alpha, toon.transparent_depth_write) {
            (Alpha::Opaque, _) => Self::Opaque,
            (Alpha::Mask { .. }, _) => Self::Cutout,
            (Alpha::Blend, false) => Self::Transparent,
            (Alpha::Blend, true) => Self::TransparentWithZWrite,
        }
    }

    fn index(self) -> f32 {
        match self {
            Self::Opaque => 0.0,
            Self::Cutout => 1.0,
            Self::Transparent => 2.0,
            Self::TransparentWithZWrite => 3.0,
        }
    }

    fn queue(self, render_order: i32) -> i32 {
        match self {
            Self::Opaque => GEOMETRY_QUEUE,
            Self::Cutout => ALPHA_TEST_QUEUE,
            Self::Transparent => TRANSPARENT_QUEUE + render_order.clamp(-9, 0),
            Self::TransparentWithZWrite => TRANSPARENT_DEPTH_QUEUE + render_order.clamp(0, 9),
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Self::Opaque => "Opaque",
            Self::Cutout => "TransparentCutout",
            Self::Transparent | Self::TransparentWithZWrite => "Transparent",
        }
    }
}

pub(super) fn write(model: &Model, ctx: &mut ExportCtx<'_>) -> Vec<raw::MaterialProperties> {
    model
        .materials
        .iter()
        .map(|material| match material.shading {
            Shading::Toon(toon) => mtoon(material, &toon, ctx),
            Shading::Pbr(_) => raw::MaterialProperties {
                name: Some(material.name.clone()),
                shader: Some(GLTF_SHADER.to_owned()),
                render_queue: Some(-1),
                ..raw::MaterialProperties::default()
            },
        })
        .collect()
}

fn mtoon(material: &Material, toon: &Toon, ctx: &mut ExportCtx<'_>) -> raw::MaterialProperties {
    warn_losses(material, toon, ctx);
    let mode = RenderMode::of(material, toon);
    let (shade_shift, shade_toony) = legacy_shading(toon.shading_toony, toon.shading_shift);
    let (outline_mode, outline_width) = match toon.outline.width {
        OutlineWidth::None => (0.0, 0.0),
        OutlineWidth::World(width) => (1.0, width / raw::OUTLINE_WORLD_SCALE),
        OutlineWidth::Screen(width) => (2.0, width / raw::OUTLINE_SCREEN_SCALE),
    };
    let outline_mixed = toon.outline.lighting_mix > 0.0;

    let floats: BTreeMap<String, f32> = [
        ("_Cutoff", cutoff(material)),
        ("_BumpScale", toon.normal_scale),
        ("_ReceiveShadowRate", 1.0),
        ("_ShadingGradeRate", 1.0),
        ("_ShadeShift", shade_shift),
        ("_ShadeToony", shade_toony),
        ("_LightColorAttenuation", 0.0),
        (
            "_IndirectLightIntensity",
            (1.0 - toon.gi_equalization).clamp(0.0, 1.0),
        ),
        ("_RimLightingMix", toon.rim_lighting_mix),
        ("_RimFresnelPower", toon.rim_fresnel_power),
        ("_RimLift", toon.rim_lift),
        ("_OutlineWidth", outline_width),
        ("_OutlineScaledMaxDistance", 1.0),
        ("_OutlineLightingMix", toon.outline.lighting_mix),
        ("_UvAnimScrollX", toon.scroll.x),
        ("_UvAnimScrollY", -toon.scroll.y),
        ("_UvAnimRotation", toon.spin / TAU),
        ("_MToonVersion", MTOON_VERSION),
        ("_DebugMode", 0.0),
        ("_BlendMode", mode.index()),
        ("_OutlineWidthMode", outline_mode),
        ("_OutlineColorMode", f32::from(u8::from(outline_mixed))),
        (
            "_CullMode",
            match material.double_sided {
                true => CULL_OFF,
                false => CULL_BACK,
            },
        ),
        ("_OutlineCullMode", CULL_FRONT),
        (
            "_SrcBlend",
            match mode {
                RenderMode::Opaque | RenderMode::Cutout => BLEND_ONE,
                _ => BLEND_SOURCE_ALPHA,
            },
        ),
        (
            "_DstBlend",
            match mode {
                RenderMode::Opaque | RenderMode::Cutout => BLEND_ZERO,
                _ => BLEND_ONE_MINUS_SOURCE_ALPHA,
            },
        ),
        (
            "_ZWrite",
            match mode {
                RenderMode::Transparent => 0.0,
                _ => 1.0,
            },
        ),
        (
            "_AlphaToMask",
            match mode {
                RenderMode::Cutout => 1.0,
                _ => 0.0,
            },
        ),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value))
    .collect();

    let textures = &material.textures;
    let texture_slots: [(&str, Option<TextureId>); 8] = [
        ("_MainTex", textures.base_color),
        ("_ShadeTexture", textures.shade),
        ("_BumpMap", textures.normal),
        ("_RimTexture", textures.rim),
        ("_SphereAdd", textures.matcap),
        ("_EmissionMap", textures.emissive),
        ("_OutlineWidthTexture", textures.outline_width),
        ("_UvAnimMaskTexture", textures.uv_animation_mask),
    ];
    let transform = material.uv_transform;
    let placement = vec![
        transform.offset.x,
        1.0 - transform.scale.y - transform.offset.y,
        transform.scale.x,
        transform.scale.y,
    ];

    let mut vectors: BTreeMap<String, Vec<f32>> = [
        ("_Color", gamma_color(material.base_color)),
        ("_ShadeColor", gamma_color(toon.shade_color.extend(1.0))),
        ("_RimColor", gamma_color(toon.rim_color.extend(1.0))),
        (
            "_EmissionColor",
            material.emissive.extend(1.0).to_array().to_vec(),
        ),
        ("_OutlineColor", gamma_color(toon.outline.color.extend(1.0))),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value))
    .collect();
    vectors.insert("_MainTex".to_owned(), placement.clone());
    let mut texture_properties = BTreeMap::new();
    for (name, texture) in texture_slots {
        let Some(texture) = texture else {
            continue;
        };
        texture_properties.insert(name.to_owned(), texture.0);
        vectors.insert(name.to_owned(), placement.clone());
    }

    let keywords = [
        ("_NORMALMAP", textures.normal.is_some()),
        ("_ALPHATEST_ON", matches!(mode, RenderMode::Cutout)),
        (
            "_ALPHABLEND_ON",
            matches!(
                mode,
                RenderMode::Transparent | RenderMode::TransparentWithZWrite
            ),
        ),
        (
            "MTOON_OUTLINE_WIDTH_WORLD",
            matches!(toon.outline.width, OutlineWidth::World(_)),
        ),
        (
            "MTOON_OUTLINE_WIDTH_SCREEN",
            matches!(toon.outline.width, OutlineWidth::Screen(_)),
        ),
        (
            "MTOON_OUTLINE_COLOR_FIXED",
            toon.outline.width != OutlineWidth::None && !outline_mixed,
        ),
        (
            "MTOON_OUTLINE_COLOR_MIXED",
            toon.outline.width != OutlineWidth::None && outline_mixed,
        ),
    ];

    raw::MaterialProperties {
        name: Some(material.name.clone()),
        shader: Some(MTOON.to_owned()),
        render_queue: Some(mode.queue(toon.render_order)),
        float_properties: floats,
        vector_properties: vectors,
        texture_properties,
        keyword_map: keywords
            .into_iter()
            .filter(|(_, enabled)| *enabled)
            .map(|(name, _)| (name.to_owned(), Value::Bool(true)))
            .collect(),
        tag_map: BTreeMap::from([(
            "RenderType".to_owned(),
            Value::String(mode.tag().to_owned()),
        )]),
    }
}

fn cutoff(material: &Material) -> f32 {
    match material.alpha {
        Alpha::Mask { cutoff } => cutoff,
        Alpha::Opaque | Alpha::Blend => 0.5,
    }
}

fn gamma_color(color: Vec4) -> Vec<f32> {
    vec![gamma(color.x), gamma(color.y), gamma(color.z), color.w]
}

pub(super) fn legacy_shading(toony: f32, shift: f32) -> (f32, f32) {
    let lowest = toony - shift - 1.0;
    let highest = 1.0 - toony - shift;
    let legacy_toony = match (1.0 - lowest).abs() < f32::EPSILON {
        true => 0.9,
        false => (1.0 - highest) / (1.0 - lowest),
    };
    (lowest.clamp(-1.0, 1.0), legacy_toony.clamp(0.0, 1.0))
}

fn warn_losses(material: &Material, toon: &Toon, ctx: &mut ExportCtx<'_>) {
    let (shift, toony) = legacy_shading(toon.shading_toony, toon.shading_shift);
    let lowest = shift;
    let highest = 1.0 + (shift - 1.0) * toony;
    let reached_toony = (2.0 - (highest - lowest)) * 0.5;
    let reached_shift = -(highest + lowest) * 0.5;
    let losses = [
        (
            material.textures.shading_shift.is_some(),
            "shading shift texture",
        ),
        (
            material.textures.matcap.is_some()
                && !toon.matcap_color.abs_diff_eq(Vec3::ONE, DEVIATION),
            "matcap colour",
        ),
        (material.uv_transform.rotation != 0.0, "texture rotation"),
        (
            (reached_toony - toon.shading_toony).abs() > DEVIATION
                || (reached_shift - toon.shading_shift).abs() > DEVIATION,
            "shading toony and shift range",
        ),
        (
            material.alpha == Alpha::Blend
                && toon.render_order
                    != match toon.transparent_depth_write {
                        true => toon.render_order.clamp(0, 9),
                        false => toon.render_order.clamp(-9, 0),
                    },
            "render queue offset",
        ),
    ];
    let names: Vec<&str> = losses
        .into_iter()
        .filter(|(lost, _)| *lost)
        .map(|(_, name)| name)
        .collect();
    if !names.is_empty() {
        ctx.warn(Warning::Unsupported(format!(
            "material \"{}\": MToon 0.x cannot store the {}",
            material.name,
            names.join(", ")
        )));
    }
}
