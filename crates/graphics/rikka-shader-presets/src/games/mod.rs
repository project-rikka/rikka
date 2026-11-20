mod genshin;
mod starrail;
mod zenless;

use rikka_shader_graph::{
    Arg, Builder, Constant, Function, FunctionPort, Input, Interpolation, NodeId, NodeKind, Op,
    ParameterDecl, Ramp, Stop, ValueType, Wire,
};
use rikka_slang::{Literal, ParameterKind, Role, TextureKind};

use crate::{Preset, import};

pub(crate) fn presets() -> Vec<Preset> {
    [genshin::presets(), starrail::presets(), zenless::presets()].concat()
}

pub(super) const BASE: &str = "Base";
pub(super) const SHADE: &str = "Shade";
pub(super) const NORMAL: &str = "Normal";
pub(super) const EMISSION: &str = "Emission";
pub(super) const OUTLINE: &str = "Outline";
pub(super) const SPECULAR: &str = "Specular";
pub(super) const RIM: &str = "Rim";
pub(super) const FACE: &str = "Face";

pub(super) const BASE_COLOR: &str = "Base Color";
pub(super) const NORMAL_MAP: &str = "Normal Map";
pub(super) const OPTIONAL_MAP: &str = "Optional Map";
pub(super) const INCOMING_LIGHT: &str = "Incoming Light";
pub(super) const FLAT_AMBIENT: &str = "Flat Ambient";
pub(super) const FACE_SHADOW: &str = "Face Shadow";

pub(super) struct Imports {
    pub base_color: Wire,
    pub base_texture: Wire,
    pub shade_color: Wire,
    pub emission_color: Wire,
    pub emission_texture: Wire,
    pub normal_texture: Wire,
    pub normal_scale: Wire,
    pub outline_width: Wire,
    pub outline_color: Wire,
}

pub(super) struct Defaults {
    pub shade: [f32; 3],
    pub outline: [f32; 3],
    pub outline_width: f32,
}

pub(super) fn imports(b: &mut Builder<'_>, defaults: &Defaults) -> Imports {
    Imports {
        base_color: b.parameter(
            ParameterDecl::new(
                import::BASE_COLOR,
                ParameterKind::Color {
                    alpha: true,
                    srgb: true,
                },
            )
            .with_default(Literal::Vec4([1.0, 1.0, 1.0, 1.0]))
            .with_group(BASE)
            .with_label("Base colour")
            .with_role(Role::Albedo),
        ),
        base_texture: b.parameter(
            map(
                import::BASE_TEXTURE,
                TextureKind::Colour,
                BASE,
                "Base texture",
            )
            .with_role(Role::AlbedoTexture),
        ),
        shade_color: b.parameter(color(
            import::SHADE_COLOR,
            defaults.shade,
            SHADE,
            "Shade colour",
        )),
        emission_color: b.parameter(
            ParameterDecl::new(
                import::EMISSION_COLOR,
                ParameterKind::Color {
                    alpha: false,
                    srgb: false,
                },
            )
            .with_default(Literal::Vec3([0.0, 0.0, 0.0]))
            .with_soft_range(0.0, 16.0)
            .with_group(EMISSION)
            .with_label("Emission colour")
            .with_role(Role::Emission),
        ),
        emission_texture: b.parameter(
            map(
                import::EMISSION_TEXTURE,
                TextureKind::Colour,
                EMISSION,
                "Emission texture",
            )
            .with_role(Role::EmissionTexture),
        ),
        normal_texture: b.parameter(map(
            import::NORMAL_TEXTURE,
            TextureKind::Normal,
            NORMAL,
            "Normal texture",
        )),
        normal_scale: b.parameter(float(
            import::NORMAL_SCALE,
            1.0,
            (0.0, 2.0),
            NORMAL,
            "Normal strength",
        )),
        outline_width: b.parameter(float(
            import::OUTLINE_WIDTH,
            defaults.outline_width,
            (0.0, 0.01),
            OUTLINE,
            "Outline width (m)",
        )),
        outline_color: b.parameter(color(
            import::OUTLINE_COLOR,
            defaults.outline,
            OUTLINE,
            "Outline colour",
        )),
    }
}

pub(super) fn float(
    name: &str,
    default: f32,
    soft: (f32, f32),
    group: &str,
    label: &str,
) -> ParameterDecl {
    ParameterDecl::float(name, default)
        .with_soft_range(soft.0, soft.1)
        .with_group(group)
        .with_label(label)
}

pub(super) fn unit(name: &str, default: f32, group: &str, label: &str) -> ParameterDecl {
    ParameterDecl::float(name, default)
        .with_range(0.0, 1.0)
        .with_group(group)
        .with_label(label)
}

pub(super) fn color(name: &str, default: [f32; 3], group: &str, label: &str) -> ParameterDecl {
    ParameterDecl::new(
        name,
        ParameterKind::Color {
            alpha: false,
            srgb: true,
        },
    )
    .with_default(Literal::Vec3(default))
    .with_group(group)
    .with_label(label)
}

pub(super) fn map(name: &str, kind: TextureKind, group: &str, label: &str) -> ParameterDecl {
    ParameterDecl::texture(name, kind)
        .with_group(group)
        .with_label(label)
}

pub(super) fn toggle(name: &str, on: bool, group: &str, label: &str) -> ParameterDecl {
    ParameterDecl::new(name, ParameterKind::Bool)
        .with_default(Literal::Bool(on))
        .with_group(group)
        .with_label(label)
}

pub(super) fn choice(
    name: &str,
    options: &[&str],
    chosen: u32,
    group: &str,
    label: &str,
) -> ParameterDecl {
    ParameterDecl::new(
        name,
        ParameterKind::Enum {
            options: options.iter().map(|option| (*option).to_owned()).collect(),
        },
    )
    .with_default(Literal::Enum(chosen))
    .with_group(group)
    .with_label(label)
}

pub(super) fn integer(name: &str, default: i32, group: &str, label: &str) -> ParameterDecl {
    ParameterDecl::new(name, ParameterKind::Int)
        .with_default(Literal::Int(default))
        .with_range(0.0, 15.0)
        .with_group(group)
        .with_label(label)
}

pub(super) fn group(
    b: &mut Builder<'_>,
    name: &str,
    doc: &str,
    inputs: &[(&str, ValueType, Option<Constant>)],
    outputs: &[(&str, ValueType)],
    body: impl FnOnce(&mut Builder<'_>),
) {
    let function = Function {
        inputs: inputs
            .iter()
            .map(|(port, ty, default)| {
                let port = FunctionPort::new(*port, *ty);
                match default {
                    Some(constant) => port.with_default(*constant),
                    None => port,
                }
            })
            .collect(),
        outputs: outputs
            .iter()
            .map(|(port, ty)| FunctionPort::new(*port, *ty))
            .collect(),
        doc: doc.to_owned(),
        ..Function::default()
    };
    b.function(name, function, |b| body(b));
}

pub(super) fn node<const N: usize>(
    b: &mut Builder<'_>,
    id: &str,
    inputs: [(&str, Arg); N],
) -> NodeId {
    b.external(id, inputs)
}

pub(super) fn call<const N: usize>(
    b: &mut Builder<'_>,
    function: &str,
    inputs: [(&str, Arg); N],
) -> NodeId {
    b.call(function, inputs)
}

pub(super) fn one_minus(b: &mut Builder<'_>, value: impl Into<Arg>) -> Wire {
    b.op(Op::OneMinus, [value.into()])
}

pub(super) fn maximum(b: &mut Builder<'_>, a: impl Into<Arg>, c: impl Into<Arg>) -> Wire {
    b.op(Op::Maximum, [a.into(), c.into()])
}

pub(super) fn power(b: &mut Builder<'_>, a: impl Into<Arg>, exponent: impl Into<Arg>) -> Wire {
    b.op(Op::Power, [a.into(), exponent.into()])
}

#[derive(Clone, Copy)]
pub(super) enum Compare {
    Less,
    Greater,
    Equal,
}

pub(super) fn compare(
    b: &mut Builder<'_>,
    a: impl Into<Arg>,
    threshold: impl Into<Arg>,
    mode: Compare,
) -> Wire {
    let mode = match mode {
        Compare::Less => 0.0,
        Compare::Greater => 2.0,
        Compare::Equal => 4.0,
    };
    let compared = b.with(
        NodeKind::Op(Op::Compare),
        [
            ("a", a.into()),
            ("b", threshold.into()),
            ("epsilon", 0.25.into()),
            ("mode", mode.into()),
        ],
    );
    b.out(compared, "value")
}

pub(super) fn combine(
    b: &mut Builder<'_>,
    x: impl Into<Arg>,
    y: impl Into<Arg>,
    z: impl Into<Arg>,
) -> Wire {
    let combined = b.with(
        NodeKind::Op(Op::Combine),
        [("x", x.into()), ("y", y.into()), ("z", z.into())],
    );
    b.out(combined, "xyz")
}

pub(super) fn edge_ramp(b: &mut Builder<'_>, position: impl Into<Arg>, edges: [f32; 2]) -> Wire {
    let stop = |position: f32, value: f32| Stop {
        position,
        color: [value, value, value, 1.0],
    };
    let ramp = Ramp {
        stops: vec![
            stop(0.0, 0.0),
            stop(edges[0], 0.0),
            stop(edges[1], 1.0),
            stop(1.0, 1.0),
        ],
        interpolation: Interpolation::Linear,
    };
    let value = b.ramp(ramp, position);
    b.swizzle(value, "x")
}

pub(super) fn common(b: &mut Builder<'_>) {
    let rgba = Some(Constant::vec4(1.0, 1.0, 1.0, 1.0));
    group(
        b,
        BASE_COLOR,
        "The surface colour: colour (linear RGBA tint) × texture (colour map; unbound samples white) at the mesh uv; albedo is its rgb, alpha its alpha.",
        &[
            ("color", ValueType::Float4, rgba),
            ("texture", ValueType::Texture, None),
        ],
        &[("albedo", ValueType::Float3), ("alpha", ValueType::Float)],
        |b| {
            let color = b.function_input("color");
            let texture = b.function_input("texture");
            let sampled = b.texture(TextureKind::Colour, [("texture", texture.into())]);
            let sampled = b.out(sampled, "rgba");
            let rgba = b.multiply(color, sampled);
            let albedo = b.swizzle(rgba.clone(), "rgb");
            let alpha = b.swizzle(rgba, "a");
            b.function_output("albedo", albedo);
            b.function_output("alpha", alpha);
        },
    );
    group(
        b,
        NORMAL_MAP,
        "The shading normal: texture (tangent-space normal map; unbound keeps the mesh normal) tilted by strength; normal is in world space.",
        &[
            ("texture", ValueType::Texture, None),
            ("strength", ValueType::Float, Some(Constant::scalar(1.0))),
        ],
        &[("normal", ValueType::Float3)],
        |b| {
            let texture = b.function_input("texture");
            let strength = b.function_input("strength");
            let tangent = b.texture(TextureKind::Normal, [("texture", texture.into())]);
            let tangent = b.out(tangent, "normal");
            let normal = b.op(Op::NormalMap, [tangent.into(), strength.into()]);
            b.function_output("normal", normal);
        },
    );
    group(
        b,
        OPTIONAL_MAP,
        "A painted data map that may be missing: value is texture (linear RGBA, read at uv) when one is bound, else fallback (the recipe's neutral texel); bound is 1 when a texture is bound.",
        &[
            ("texture", ValueType::Texture, None),
            ("fallback", ValueType::Float4, rgba),
            ("uv", ValueType::Float2, None),
        ],
        &[("value", ValueType::Float4), ("bound", ValueType::Float)],
        |b| {
            let texture = b.function_input("texture");
            let fallback = b.function_input("fallback");
            let uv = b.function_input("uv");
            let bound = node(b, "texture.bound", [("texture", texture.clone().into())]);
            let bound = b.out(bound, "value");
            let sampled = b.texture(
                TextureKind::Data,
                [("texture", texture.into()), ("uv", uv.into())],
            );
            let sampled = b.out(sampled, "rgba");
            let value = b.lerp(fallback, sampled, bound.clone());
            b.function_output("value", value);
            b.function_output("bound", bound);
        },
    );
    group(
        b,
        INCOMING_LIGHT,
        "The light being added: color = radiance × falloff, leading = 1 for the key light and 0 for the others, direction towards it.",
        &[],
        &[
            ("color", ValueType::Float3),
            ("leading", ValueType::Float),
            ("direction", ValueType::Float3),
        ],
        |b| {
            let light = node(b, "light.sample", []);
            let radiance = b.out(light, "color");
            let falloff = b.out(light, "attenuation");
            let color = b.multiply(radiance, falloff);
            let leading = b.out(light, "leading");
            let direction = b.out(light, "direction");
            b.function_output("color", color);
            b.function_output("leading", leading);
            b.function_output("direction", direction);
        },
    );
    group(
        b,
        FLAT_AMBIENT,
        "Light from the surroundings without shading, so the cel bands stay clean: albedo × the mean of the sky above and below (or the frame ambient) × strength.",
        &[
            ("albedo", ValueType::Float3, rgba),
            ("strength", ValueType::Float, Some(Constant::scalar(0.5))),
        ],
        &[("ambient", ValueType::Float3)],
        |b| {
            let albedo = b.function_input("albedo");
            let strength = b.function_input("strength");
            let sky = node(b, "light.environment", [("equalization", 1.0.into())]);
            let sky = b.out(sky, "equalized");
            let lit = b.multiply(albedo, sky);
            let ambient = b.multiply(lit, strength);
            b.function_output("ambient", ambient);
        },
    );
}

pub(super) struct Base {
    pub albedo: Wire,
    pub alpha: Wire,
    pub normal: Wire,
    pub uv: Wire,
}

pub(super) fn base(b: &mut Builder<'_>, imports: &Imports) -> Base {
    let surface = call(
        b,
        BASE_COLOR,
        [
            ("color", imports.base_color.clone().into()),
            ("texture", imports.base_texture.clone().into()),
        ],
    );
    let normal = call(
        b,
        NORMAL_MAP,
        [
            ("texture", imports.normal_texture.clone().into()),
            ("strength", imports.normal_scale.clone().into()),
        ],
    );
    Base {
        albedo: b.out(surface, "albedo"),
        alpha: b.out(surface, "alpha"),
        normal: b.out(normal, "normal"),
        uv: b.input(Input::Uv),
    }
}

pub(super) fn optional_map(
    b: &mut Builder<'_>,
    texture: &Wire,
    fallback: [f32; 4],
    uv: &Wire,
) -> (Wire, Wire) {
    let map = call(
        b,
        OPTIONAL_MAP,
        [
            ("texture", texture.into()),
            ("fallback", fallback.into()),
            ("uv", uv.into()),
        ],
    );
    (b.out(map, "value"), b.out(map, "bound"))
}

pub(super) struct Incoming {
    pub color: Wire,
    pub leading: Wire,
}

pub(super) fn incoming(b: &mut Builder<'_>) -> Incoming {
    let light = call(b, INCOMING_LIGHT, []);
    Incoming {
        color: b.out(light, "color"),
        leading: b.out(light, "leading"),
    }
}

pub(super) fn flat_ambient(b: &mut Builder<'_>, albedo: &Wire, strength: &Wire) -> Wire {
    let ambient = call(
        b,
        FLAT_AMBIENT,
        [("albedo", albedo.into()), ("strength", strength.into())],
    );
    b.out(ambient, "ambient")
}

pub(super) fn ambient_strength(b: &mut Builder<'_>) -> Wire {
    b.parameter(float(
        "ambient_strength",
        0.5,
        (0.0, 2.0),
        SHADE,
        "Ambient strength",
    ))
}

pub(super) fn vertex_controls(b: &mut Builder<'_>) -> Wire {
    b.parameter(toggle(
        "vertex_controls",
        false,
        BASE,
        "Vertex colours hold game control data",
    ))
}

pub(super) fn alpha_use(b: &mut Builder<'_>, second: &str) -> Wire {
    b.parameter(choice(
        "alpha_use",
        &["transparency", second],
        0,
        BASE,
        "Base texture alpha",
    ))
}

pub(super) fn face_shadow_group(b: &mut Builder<'_>) {
    group(
        b,
        FACE_SHADOW,
        "Face shadow from the head-plane light: with a face map bound, the SDF channel (sdf, mirrored = its value at 1-u) against 1 - HL(forward·light) with mirror (0 mirrored when the light is on the face's left, 1 on its right) and softness; without one, a procedural face shadow (in head-radius units). lit is 1 where the face is lit; facing = HL(forward·light).",
        &[
            ("sdf", ValueType::Float, Some(Constant::scalar(0.5))),
            ("mirrored", ValueType::Float, Some(Constant::scalar(0.5))),
            ("bound", ValueType::Float, Some(Constant::scalar(0.0))),
            ("mirror", ValueType::Float, Some(Constant::scalar(0.0))),
            ("softness", ValueType::Float, Some(Constant::scalar(0.02))),
        ],
        &[("lit", ValueType::Float), ("facing", ValueType::Float)],
        |b| {
            let sdf = b.function_input("sdf");
            let mirrored = b.function_input("mirrored");
            let bound = b.function_input("bound");
            let mirror = b.function_input("mirror");
            let softness = b.function_input("softness");
            let mapped = node(
                b,
                "npr.face_sdf",
                [
                    ("sdf", sdf.into()),
                    ("mirrored", mirrored.into()),
                    ("mirror", mirror.into()),
                    ("softness", softness.into()),
                ],
            );
            let mapped = b.out(mapped, "lit");
            let procedural = node(b, "npr.face_procedural", []);
            let procedural = b.out(procedural, "lit");
            let lit = b.lerp(procedural, mapped, bound);
            let head = node(b, "npr.head_light", []);
            let facing = b.out(head, "facing");
            b.function_output("lit", lit);
            b.function_output("facing", facing);
        },
    );
}

pub(super) struct OutlineShape {
    pub width: Wire,
    pub direction: Wire,
    pub depth: Wire,
}

pub(super) struct OutlineStyle {
    pub mode: f32,
    pub distances: [f32; 3],
    pub scales: [f32; 3],
    pub flatten_z: f32,
    pub depth_mode: f32,
    pub depth_offset: f32,
}

pub(super) fn outline_shape(
    b: &mut Builder<'_>,
    width: &Wire,
    multiplier: &Wire,
    vertex_controls: &Wire,
    channel: f32,
    style: &OutlineStyle,
) -> OutlineShape {
    let channel = b.multiply(vertex_controls, channel);
    let sized = node(
        b,
        "npr.outline_width",
        [
            ("width", width.into()),
            ("mode", style.mode.into()),
            ("near", style.distances[0].into()),
            ("middle", style.distances[1].into()),
            ("far", style.distances[2].into()),
            ("near_scale", style.scales[0].into()),
            ("middle_scale", style.scales[1].into()),
            ("far_scale", style.scales[2].into()),
            ("channel", channel.into()),
            ("multiplier", multiplier.into()),
        ],
    );
    let direction = node(
        b,
        "npr.outline_direction",
        [
            ("source", 0.0.into()),
            ("flatten", 1.0.into()),
            ("flatten_z", style.flatten_z.into()),
        ],
    );
    let depth = node(
        b,
        "npr.outline_depth",
        [
            ("offset", style.depth_offset.into()),
            ("mode", style.depth_mode.into()),
        ],
    );
    OutlineShape {
        width: b.out(sized, "value"),
        direction: b.out(direction, "value"),
        depth: b.out(depth, "value"),
    }
}
