use rikka_shader_graph::{
    Arg, Builder, DirectionSpace, Input, NodeId, NodeKind, Op, ParameterDecl, PositionSpace,
    ViewSpace, Wire,
};
use rikka_slang::{Literal, ParameterKind, Preset, TextureKind};

pub(super) const NITS_PER_UNIT: f32 = 32_000.0;

pub(super) fn color(name: &str, value: [f32; 3]) -> ParameterDecl {
    ParameterDecl::new(
        name,
        ParameterKind::Color {
            alpha: false,
            srgb: true,
        },
    )
    .with_default(Literal::Vec3(value))
}

pub(super) fn color_alpha(name: &str, value: [f32; 4]) -> ParameterDecl {
    ParameterDecl::new(
        name,
        ParameterKind::Color {
            alpha: true,
            srgb: true,
        },
    )
    .with_default(Literal::Vec4(value))
}

pub(super) fn vector2(name: &str, value: [f32; 2]) -> ParameterDecl {
    ParameterDecl::new(name, ParameterKind::Vec2).with_default(Literal::Vec2(value))
}

pub(super) fn vector3(name: &str, value: [f32; 3]) -> ParameterDecl {
    ParameterDecl::new(name, ParameterKind::Vec3).with_default(Literal::Vec3(value))
}

pub(super) fn unit(name: &str, value: f32) -> ParameterDecl {
    ParameterDecl::float(name, value).with_range(0.0, 1.0)
}

pub(super) fn amount(name: &str, value: f32, low: f32, high: f32) -> ParameterDecl {
    ParameterDecl::float(name, value).with_soft_range(low, high)
}

pub(super) fn toggle(name: &str, on: bool) -> ParameterDecl {
    ParameterDecl::new(name, ParameterKind::Bool).with_default(Literal::Bool(on))
}

pub(super) fn options(name: &str, choices: &[&str], chosen: u32) -> ParameterDecl {
    ParameterDecl::new(
        name,
        ParameterKind::Enum {
            options: choices.iter().map(|choice| (*choice).to_owned()).collect(),
        },
    )
    .with_default(Literal::Enum(chosen))
}

pub(super) fn texture(name: &str, kind: TextureKind) -> ParameterDecl {
    ParameterDecl::texture(name, kind)
}

pub(super) fn preset(name: &str, value: Literal) -> Preset {
    Preset {
        name: name.to_owned(),
        value,
    }
}

pub(super) fn presets(mut parameter: ParameterDecl, named: &[(&str, Literal)]) -> ParameterDecl {
    parameter
        .presets
        .extend(named.iter().map(|(name, value)| preset(name, *value)));
    parameter
}

pub(super) fn sample(builder: &mut Builder<'_>, kind: TextureKind, texture: &Wire) -> NodeId {
    builder.texture(kind, [("texture", texture.into())])
}

pub(super) fn sample_at(
    builder: &mut Builder<'_>,
    kind: TextureKind,
    texture: &Wire,
    uv: impl Into<Arg>,
) -> NodeId {
    builder.texture(kind, [("texture", texture.into()), ("uv", uv.into())])
}

pub(super) fn rgba(builder: &mut Builder<'_>, texture: &Wire) -> Wire {
    let node = sample(builder, TextureKind::Colour, texture);
    builder.out(node, "rgba")
}

pub(super) fn data(builder: &mut Builder<'_>, texture: &Wire) -> Wire {
    let node = sample(builder, TextureKind::Data, texture);
    builder.out(node, "rgba")
}

pub(super) fn channel(builder: &mut Builder<'_>, value: &Wire, mask: &str) -> Wire {
    builder.swizzle(value, mask)
}

pub(super) fn node<const N: usize>(
    builder: &mut Builder<'_>,
    id: &str,
    inputs: [(&str, Arg); N],
) -> NodeId {
    builder.external(id, inputs)
}

pub(super) fn value<const N: usize>(
    builder: &mut Builder<'_>,
    id: &str,
    output: &str,
    inputs: [(&str, Arg); N],
) -> Wire {
    let node = builder.external(id, inputs);
    builder.out(node, output)
}

pub(super) fn combine(builder: &mut Builder<'_>, parts: [Arg; 3]) -> Wire {
    let [x, y, z] = parts;
    let node = builder.with(NodeKind::Op(Op::Combine), [("x", x), ("y", y), ("z", z)]);
    builder.out(node, "xyz")
}

pub(super) fn combine2(builder: &mut Builder<'_>, x: impl Into<Arg>, y: impl Into<Arg>) -> Wire {
    let node = builder.with(
        NodeKind::Op(Op::Combine),
        [("x", x.into()), ("y", y.into())],
    );
    builder.out(node, "xy")
}

pub(super) fn one_minus(builder: &mut Builder<'_>, value: impl Into<Arg>) -> Wire {
    builder.op(Op::OneMinus, [value.into()])
}

pub(super) fn normalize(builder: &mut Builder<'_>, value: impl Into<Arg>) -> Wire {
    builder.op(Op::Normalize, [value.into()])
}

pub(super) fn maximum(builder: &mut Builder<'_>, a: impl Into<Arg>, b: impl Into<Arg>) -> Wire {
    builder.op(Op::Maximum, [a.into(), b.into()])
}

pub(super) fn divide(builder: &mut Builder<'_>, a: impl Into<Arg>, b: impl Into<Arg>) -> Wire {
    builder.op(Op::Divide, [a.into(), b.into()])
}

pub(super) fn normal(builder: &mut Builder<'_>) -> Wire {
    builder.input(Input::Normal(DirectionSpace::World))
}

pub(super) fn view(builder: &mut Builder<'_>) -> Wire {
    builder.input(Input::ViewDirection(ViewSpace::World))
}

pub(super) fn position(builder: &mut Builder<'_>) -> Wire {
    builder.input(Input::Position(PositionSpace::World))
}

pub(super) fn uv(builder: &mut Builder<'_>) -> Wire {
    builder.input(Input::Uv)
}

pub(super) fn emission(builder: &mut Builder<'_>, color: impl Into<Arg>) -> Wire {
    builder.shader(NodeKind::Emission, [("color", color.into())])
}

pub(super) fn add_shader(builder: &mut Builder<'_>, a: &Wire, b: &Wire) -> Wire {
    builder.shader(NodeKind::AddShader, [("a", a.into()), ("b", b.into())])
}

pub(super) fn mix_shader(
    builder: &mut Builder<'_>,
    a: &Wire,
    b: &Wire,
    factor: impl Into<Arg>,
) -> Wire {
    builder.shader(
        NodeKind::MixShader,
        [("a", a.into()), ("b", b.into()), ("factor", factor.into())],
    )
}

pub(super) fn lighting(builder: &mut Builder<'_>, light: &Wire, ambient: &Wire) -> Wire {
    builder.shader(
        NodeKind::Lighting,
        [("light", light.into()), ("ambient", ambient.into())],
    )
}

pub(super) fn shader_color(builder: &mut Builder<'_>, shader: &Wire) -> Wire {
    let node = builder.with(NodeKind::ShaderToRgb, [("shader", shader.into())]);
    builder.out(node, "color")
}
