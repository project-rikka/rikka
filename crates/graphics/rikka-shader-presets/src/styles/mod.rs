mod blocks;
mod endfield;
mod hybrid;
mod kit;
mod looks;
mod realistic;
mod toon;

use rikka_shader_graph::{Builder, Op, ParameterDecl, Wire};
use rikka_slang::{Role, TextureKind};

use crate::{Preset, import};

pub(crate) fn presets() -> Vec<Preset> {
    [
        realistic::presets(),
        endfield::presets(),
        hybrid::presets(),
        toon::presets(),
        looks::presets(),
    ]
    .concat()
}

pub(super) fn base_color(value: [f32; 4]) -> ParameterDecl {
    kit::color_alpha(import::BASE_COLOR, value)
        .with_group("base")
        .with_label("Colour")
        .with_role(Role::Albedo)
}

pub(super) fn base_texture() -> ParameterDecl {
    kit::texture(import::BASE_TEXTURE, TextureKind::Colour)
        .with_group("base")
        .with_label("Texture")
        .with_role(Role::AlbedoTexture)
}

pub(super) fn shade_color(value: [f32; 3]) -> ParameterDecl {
    kit::color(import::SHADE_COLOR, value)
        .with_group("shade")
        .with_label("Shade colour")
}

pub(super) fn emission_color() -> ParameterDecl {
    kit::color(import::EMISSION_COLOR, [0.0, 0.0, 0.0])
        .with_group("emission")
        .with_label("Colour")
        .with_role(Role::Emission)
}

pub(super) fn emission_texture() -> ParameterDecl {
    kit::texture(import::EMISSION_TEXTURE, TextureKind::Colour)
        .with_group("emission")
        .with_label("Texture")
        .with_role(Role::EmissionTexture)
}

pub(super) fn normal_texture() -> ParameterDecl {
    kit::texture(import::NORMAL_TEXTURE, TextureKind::Normal)
        .with_group("geometry")
        .with_label("Normal map")
}

pub(super) fn normal_scale() -> ParameterDecl {
    kit::amount(import::NORMAL_SCALE, 1.0, 0.0, 2.0)
        .with_group("geometry")
        .with_label("Normal strength")
}

pub(super) fn outline_width(metres: f32) -> ParameterDecl {
    kit::amount(import::OUTLINE_WIDTH, metres, 0.0, 0.01)
        .with_group("outline")
        .with_label("Width")
}

pub(super) fn outline_color(value: [f32; 3]) -> ParameterDecl {
    kit::color(import::OUTLINE_COLOR, value)
        .with_group("outline")
        .with_label("Colour")
}

pub(super) struct Albedo {
    pub(super) rgb: Wire,
    pub(super) alpha: Wire,
}

pub(super) fn albedo(builder: &mut Builder<'_>, value: [f32; 4]) -> Albedo {
    builder.frame("Base colour", |builder| {
        let color = builder.parameter(base_color(value));
        let texture = builder.parameter(base_texture());
        let texel = kit::rgba(builder, &texture);
        let base = builder.multiply(color, texel);
        Albedo {
            rgb: kit::channel(builder, &base, "xyz"),
            alpha: kit::channel(builder, &base, "w"),
        }
    })
}

pub(super) fn mapped_normal(builder: &mut Builder<'_>) -> Wire {
    builder.frame("Normal map", |builder| {
        let texture = builder.parameter(normal_texture());
        let strength = builder.parameter(normal_scale());
        let texel = kit::sample(builder, TextureKind::Normal, &texture);
        let tangent = builder.out(texel, "normal");
        builder.op(Op::NormalMap, [tangent.into(), strength.into()])
    })
}

pub(super) fn emitted(builder: &mut Builder<'_>) -> Wire {
    builder.frame("Emission", |builder| {
        let color = builder.parameter(emission_color());
        let texture = builder.parameter(emission_texture());
        let texel = kit::rgba(builder, &texture);
        let texel = kit::channel(builder, &texel, "xyz");
        builder.multiply(color, texel)
    })
}

pub(super) struct Outline {
    pub(super) shader: Wire,
    pub(super) width: Wire,
}

pub(super) fn outline(
    builder: &mut Builder<'_>,
    albedo: &Wire,
    metres: f32,
    value: [f32; 3],
) -> Outline {
    builder.frame("Outline", |builder| {
        let width = builder.parameter(outline_width(metres));
        let color = builder.parameter(outline_color(value));
        let blend = builder.parameter(
            kit::unit("outline_albedo", 0.5)
                .with_group("outline")
                .with_label("Albedo tint"),
        );
        let tint = builder.lerp([1.0, 1.0, 1.0], albedo, blend);
        let tinted = builder.multiply(color, tint);
        Outline {
            shader: kit::emission(builder, tinted),
            width,
        }
    })
}
