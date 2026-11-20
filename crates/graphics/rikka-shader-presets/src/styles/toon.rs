use rikka_shader_graph::{Builder, Error, Graph, GraphKind, Input, Library, Op};
use rikka_slang::TextureKind;

use super::blocks;
use super::kit;
use crate::{Family, Preset};

pub(super) fn presets() -> Vec<Preset> {
    vec![Preset {
        key: "toon_basic",
        title: "Toon",
        summary: "UTS and lilToon-style three tones: base, first and second shade with steps and feathers and shade maps, a masked high colour, a light-side rim, matcap, angel ring for hair, emission and an outline with a width map.",
        family: Family::Stylized,
        build: toon_basic,
    }]
}

fn toon_basic(library: &Library) -> Result<Graph, Error> {
    let mut b = Builder::new(GraphKind::Material, library);
    blocks::define_lit(&mut b);
    blocks::define_ambient(&mut b);
    let albedo = super::albedo(&mut b, [1.0, 1.0, 1.0, 1.0]);
    let normal = super::mapped_normal(&mut b);
    let view = kit::view(&mut b);

    let base = b.frame("Matcap", |b| {
        let texture = b.parameter(
            kit::texture("matcap_texture", TextureKind::Colour)
                .with_group("matcap")
                .with_label("Matcap"),
        );
        let tint = b.parameter(
            kit::color("matcap_color", [1.0, 1.0, 1.0])
                .with_group("matcap")
                .with_label("Tint"),
        );
        let strength = b.parameter(
            kit::amount("matcap_strength", 0.0, 0.0, 2.0)
                .with_group("matcap")
                .with_label("Strength"),
        );
        let mode = b.parameter(
            kit::options(
                "matcap_mode",
                &["add", "multiply", "mix", "overlay", "screen"],
                0,
            )
            .with_group("matcap")
            .with_label("Blend"),
        );
        let at = kit::value(
            b,
            "npr.matcap_uv",
            "uv",
            [("normal", (&normal).into()), ("view", (&view).into())],
        );
        kit::value(
            b,
            "npr.matcap",
            "color",
            [
                ("base", (&albedo.rgb).into()),
                ("texture", texture.into()),
                ("uv", at.into()),
                ("tint", tint.into()),
                ("strength", strength.into()),
                ("mode", mode.into()),
            ],
        )
    });

    let (first, second) = b.frame("Shade colours", |b| {
        let first_color =
            b.parameter(super::shade_color([0.82, 0.74, 0.8]).with_label("First shade"));
        let first_texture = b.parameter(
            kit::texture("first_shade_texture", TextureKind::Colour)
                .with_group("shade")
                .with_label("First shade map"),
        );
        let second_color = b.parameter(
            kit::color("second_shade_color", [0.62, 0.55, 0.66])
                .with_group("shade")
                .with_label("Second shade"),
        );
        let second_texture = b.parameter(
            kit::texture("second_shade_texture", TextureKind::Colour)
                .with_group("shade")
                .with_label("Second shade map"),
        );
        let first_map = kit::rgba(b, &first_texture);
        let first_map = kit::channel(b, &first_map, "xyz");
        let first = b.multiply(&base, first_color);
        let first = b.multiply(first, first_map);
        let second_map = kit::rgba(b, &second_texture);
        let second_map = kit::channel(b, &second_map, "xyz");
        let second = b.multiply(&base, second_color);
        let second = b.multiply(second, second_map);
        (first, second)
    });

    let steps = b.frame("Shade steps", |b| {
        let group = "shade";
        [
            ("shade_step", 0.5, "First step"),
            ("shade_feather", 0.02, "First feather"),
            ("second_step", 0.25, "Second step"),
            ("second_feather", 0.02, "Second feather"),
            ("cast_shadow", 1.0, "Receive shadows"),
        ]
        .map(|(name, value, label)| {
            b.parameter(kit::unit(name, value).with_group(group).with_label(label))
        })
    });
    let [first_step, first_feather, second_step, second_feather, cast] = steps;

    let diffuse = b.frame("Three tones (per light)", |b| {
        let wrapped = kit::value(b, "npr.wrap", "value", [("normal", (&normal).into())]);
        let shadow = kit::value(b, "light.shadow", "visibility", []);
        let shadow = b.lerp(1.0, shadow, &cast);
        let value = b.multiply(wrapped, shadow);
        let to_first = kit::value(
            b,
            "npr.toon_step",
            "value",
            [
                ("value", (&value).into()),
                ("edge", first_step.into()),
                ("feather", first_feather.into()),
            ],
        );
        let to_second = kit::value(
            b,
            "npr.toon_step",
            "value",
            [
                ("value", value.into()),
                ("edge", second_step.into()),
                ("feather", second_feather.into()),
            ],
        );
        let shade = b.lerp(&second, &first, to_second);
        let tone = b.lerp(shade, &base, &to_first);
        let lit = blocks::lit(b, tone, [1.0, 1.0, 1.0]);
        (lit, to_first)
    });
    let (diffuse, lit_side) = diffuse;

    let high = b.frame("High colour (per light)", |b| {
        let color = b.parameter(
            kit::color("high_color", [1.0, 1.0, 1.0])
                .with_group("high_color")
                .with_label("Colour"),
        );
        let size = b.parameter(
            kit::unit("high_color_size", 0.08)
                .with_group("high_color")
                .with_label("Size"),
        );
        let feather = b.parameter(
            kit::amount("high_color_feather", 0.02, 0.0, 0.2)
                .with_group("high_color")
                .with_label("Feather"),
        );
        let strength = b.parameter(
            kit::amount("high_color_strength", 0.6, 0.0, 4.0)
                .with_group("high_color")
                .with_label("Strength"),
        );
        let mask_texture = b.parameter(
            kit::texture("high_color_mask_texture", TextureKind::Data)
                .with_group("high_color")
                .with_label("Mask"),
        );
        let toward = b.input(Input::LightDirection);
        let half = b.add(toward, &view);
        let half = kit::normalize(b, half);
        let facing = b.dot(&normal, half);
        let facing = kit::maximum(b, facing, 0.0);
        let edge = kit::one_minus(b, size);
        let spot = kit::value(
            b,
            "npr.toon_step",
            "value",
            [
                ("value", facing.into()),
                ("edge", edge.into()),
                ("feather", feather.into()),
            ],
        );
        let mask = kit::data(b, &mask_texture);
        let mask = kit::channel(b, &mask, "x");
        let spot = b.multiply(spot, mask);
        let spot = b.multiply(spot, strength);
        let spot = b.multiply(spot, &lit_side);
        blocks::lit(b, color, spot)
    });

    let rim = b.frame("Rim (per light)", |b| {
        let color = b.parameter(
            kit::color("rim_color", [1.0, 1.0, 1.0])
                .with_group("rim")
                .with_label("Colour"),
        );
        let power = b.parameter(
            kit::amount("rim_power", 3.0, 0.5, 16.0)
                .with_group("rim")
                .with_label("Power"),
        );
        let strength = b.parameter(
            kit::amount("rim_strength", 0.4, 0.0, 4.0)
                .with_group("rim")
                .with_label("Strength"),
        );
        let gated = b.parameter(
            kit::unit("rim_light_side", 1.0)
                .with_group("rim")
                .with_label("Light side only"),
        );
        let fresnel = kit::value(
            b,
            "npr.fresnel_rim",
            "value",
            [
                ("normal", (&normal).into()),
                ("view", (&view).into()),
                ("power", power.into()),
            ],
        );
        let toward = b.input(Input::LightDirection);
        let facing = b.dot(&normal, toward);
        let side = kit::value(
            b,
            "npr.light_side",
            "value",
            [
                ("facing", facing.into()),
                ("low", (-0.1).into()),
                ("high", 0.4.into()),
            ],
        );
        let side = b.lerp(1.0, side, gated);
        let rim = b.multiply(fresnel, side);
        let rim = b.multiply(rim, strength);
        blocks::lit(b, color, rim)
    });

    let ring = b.frame("Angel ring (per light)", |b| {
        let color = b.parameter(
            kit::color("ring_color", [1.0, 0.97, 0.92])
                .with_group("angel_ring")
                .with_label("Colour"),
        );
        let strength = b.parameter(
            kit::amount("ring_strength", 0.0, 0.0, 4.0)
                .with_group("angel_ring")
                .with_label("Strength"),
        );
        let position = b.parameter(
            kit::unit("ring_position", 0.3)
                .with_group("angel_ring")
                .with_label("Position"),
        );
        let width = b.parameter(
            kit::amount("ring_width", 0.05, 0.0, 0.3)
                .with_group("angel_ring")
                .with_label("Width"),
        );
        let follow = b.parameter(
            kit::unit("ring_follow_view", 0.5)
                .with_group("angel_ring")
                .with_label("Follow view"),
        );
        let at = kit::value(
            b,
            "npr.ring_uv",
            "uv",
            [("normal", (&normal).into()), ("offset_v", follow.into())],
        );
        let height = kit::channel(b, &at, "y");
        let offset = b.subtract(height, position);
        let distance = b.op(Op::Absolute, [offset.into()]);
        let outer = b.add(&width, 0.02);
        let fade = b.smoothstep(&width, outer, distance);
        let band = kit::one_minus(b, fade);
        let band = b.multiply(band, strength);
        let band = b.multiply(band, &lit_side);
        let tint = b.multiply(color, &base);
        let tint = b.lerp(tint, [1.0, 1.0, 1.0], 0.5);
        blocks::lit(b, tint, band)
    });

    let flat = b.parameter(
        kit::unit("ambient_flatten", 1.0)
            .with_group("shade")
            .with_label("Flat ambient"),
    );
    let ambient = b.frame("Ambient", |b| {
        blocks::ambient(
            b,
            [
                ("albedo", (&first).into()),
                ("normal", (&normal).into()),
                ("flatten", flat.into()),
            ],
        )
    });
    let light = b.add(diffuse, high);
    let light = b.add(light, rim);
    let light = b.add(light, ring);
    let cel = kit::lighting(&mut b, &light, &ambient);
    let glow = super::emitted(&mut b);
    let glow = kit::emission(&mut b, glow);
    let surface = kit::add_shader(&mut b, &cel, &glow);

    let line = super::outline(&mut b, &albedo.rgb, 0.0015, [0.25, 0.2, 0.22]);
    let width = b.frame("Outline width map", |b| {
        let texture = b.parameter(
            kit::texture("outline_width_texture", TextureKind::Data)
                .with_group("outline")
                .with_label("Width map"),
        );
        let texel = kit::data(b, &texture);
        let red = kit::channel(b, &texel, "x");
        b.multiply(&line.width, red)
    });
    b.output([
        ("surface", surface.into()),
        ("alpha", albedo.alpha.into()),
        ("outline", line.shader.into()),
        ("outline_width", width.into()),
    ]);
    b.finish()
}
