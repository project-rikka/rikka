use std::f32::consts::PI;

use rikka_shader_graph::{
    Arg, Builder, Constant, Function, FunctionPort, Input, NodeKind, Noise, Op, ValueType, Wire,
};

use super::kit;

pub(super) const CEL_BAND: &str = "Cel band";
pub(super) const BRUSH: &str = "Brush jitter";
pub(super) const LIT: &str = "Lit tone";
pub(super) const LIGHT_RIM: &str = "Light-side rim";
pub(super) const AMBIENT: &str = "Soft ambient";

fn port(name: &str, ty: ValueType) -> FunctionPort {
    FunctionPort::new(name, ty)
}

fn number(name: &str) -> FunctionPort {
    port(name, ValueType::Float)
}

fn space(name: &str) -> FunctionPort {
    port(name, ValueType::Float3)
}

fn function(inputs: Vec<FunctionPort>, outputs: Vec<FunctionPort>, doc: &str) -> Function {
    Function {
        inputs,
        outputs,
        doc: doc.to_owned(),
        body: Default::default(),
    }
}

pub(super) fn define_cel_band(builder: &mut Builder<'_>) {
    let doc = "Two tones from a light response. value is the response (half Lambert times occlusion and cast shadow, plus any jitter); band is 1 above edge and 0 below, eased over a window softness wide. tone is white on the lit band and shade in shadow, pulled towards terminator within terminator_width of the edge by terminator_strength, so the shadow edge carries a coloured fringe.";
    builder.function(
        CEL_BAND,
        function(
            vec![
                number("value"),
                number("edge").with_default(Constant::scalar(0.5)),
                number("softness").with_default(Constant::scalar(0.1)),
                space("shade").with_default(Constant::vec3(0.6, 0.6, 0.7)),
                space("terminator").with_default(Constant::vec3(1.0, 0.6, 0.5)),
                number("terminator_width").with_default(Constant::scalar(0.1)),
                number("terminator_strength").with_default(Constant::scalar(0.0)),
            ],
            vec![space("tone"), number("band")],
            doc,
        ),
        |b| {
            let value = b.function_input("value");
            let edge = b.function_input("edge");
            let softness = b.function_input("softness");
            let shade = b.function_input("shade");
            let terminator = b.function_input("terminator");
            let width = b.function_input("terminator_width");
            let strength = b.function_input("terminator_strength");
            let band = kit::value(
                b,
                "npr.toon_step",
                "value",
                [
                    ("value", (&value).into()),
                    ("edge", (&edge).into()),
                    ("feather", softness.into()),
                ],
            );
            let tone = b.lerp(shade, [1.0, 1.0, 1.0], &band);
            let offset = b.subtract(value, edge);
            let distance = b.op(Op::Absolute, [offset.into()]);
            let width = kit::maximum(b, width, 1e-4);
            let near = kit::divide(b, distance, width);
            let near = kit::one_minus(b, near);
            let near = b.saturate(near);
            let fringe = b.multiply(near, strength);
            let tone = b.lerp(tone, terminator, fringe);
            b.function_output("tone", tone)
                .function_output("band", band);
        },
    );
}

pub(super) fn cel_tone<const N: usize>(
    builder: &mut Builder<'_>,
    inputs: [(&str, Arg); N],
) -> Wire {
    let node = builder.call(CEL_BAND, inputs);
    builder.out(node, "tone")
}

pub(super) fn define_brush(builder: &mut Builder<'_>) {
    let doc = "Painterly stroke noise for shading edges: fractal noise read at uv × scale, stretched stretch times along the strokes and turned with the direction of light on screen, so the strokes swing as the light or the camera moves. jitter is (noise - 0.5) × amount, to add to a light response before it is banded.";
    builder.function(
        BRUSH,
        function(
            vec![
                port("uv", ValueType::Float2),
                space("light"),
                number("amount").with_default(Constant::scalar(0.1)),
                number("scale").with_default(Constant::scalar(8.0)),
                number("stretch").with_default(Constant::scalar(4.0)),
            ],
            vec![number("jitter")],
            doc,
        ),
        |b| {
            let at = b.function_input("uv");
            let light = b.function_input("light");
            let amount = b.function_input("amount");
            let scale = b.function_input("scale");
            let stretch = b.function_input("stretch");
            let seen = kit::value(
                b,
                "vector.view_transform",
                "value",
                [("vector", light.into())],
            );
            let y = kit::channel(b, &seen, "y");
            let x = kit::channel(b, &seen, "x");
            let angle = b.op(Op::ArcTangent2, [y.into(), x.into()]);
            let at = b.multiply(at, scale);
            let turned = b.with(
                NodeKind::Op(Op::Rotate),
                [
                    ("uv", at.into()),
                    ("center", [0.0, 0.0].into()),
                    ("angle", angle.into()),
                ],
            );
            let turned = b.out(turned, "value");
            let along = kit::combine2(b, 1.0, stretch);
            let stretched = b.multiply(turned, along);
            let noise = b.with(
                NodeKind::Fbm(Noise::Gradient),
                [("position", stretched.into()), ("octaves", 3.0.into())],
            );
            let noise = b.out(noise, "value");
            let centred = b.subtract(noise, 0.5);
            let jitter = b.multiply(centred, amount);
            b.function_output("jitter", jitter);
        },
    );
}

pub(super) fn brush<const N: usize>(builder: &mut Builder<'_>, inputs: [(&str, Arg); N]) -> Wire {
    let node = builder.call(BRUSH, inputs);
    builder.out(node, "jitter")
}

pub(super) fn define_lit(builder: &mut Builder<'_>) {
    let doc = "Lights a tone with the light being added: albedo × tone × the light's radiance and falloff ÷ π, the scale of a physically based diffuse surface, so cel parts sit next to physically based ones under one exposure. For Lighting's light input.";
    builder.function(
        LIT,
        function(
            vec![space("albedo"), space("tone")],
            vec![space("color")],
            doc,
        ),
        |b| {
            let albedo = b.function_input("albedo");
            let tone = b.function_input("tone");
            let light = kit::node(b, "light.sample", []);
            let radiance = b.out(light, "color");
            let falloff = b.out(light, "attenuation");
            let lit = b.multiply(albedo, tone);
            let lit = b.multiply(lit, radiance);
            let lit = b.multiply(lit, falloff);
            let lit = b.multiply(lit, 1.0 / PI);
            b.function_output("color", lit);
        },
    );
}

pub(super) fn lit(builder: &mut Builder<'_>, albedo: impl Into<Arg>, tone: impl Into<Arg>) -> Wire {
    let node = builder.call(LIT, [("albedo", albedo.into()), ("tone", tone.into())]);
    builder.out(node, "color")
}

pub(super) fn define_light_rim(builder: &mut Builder<'_>) {
    let doc = "A rim that only shows on the side facing the light being added: rim × smoothstep(0, 0.5, N·L) × color × strength × the light's radiance and falloff ÷ π. rim is a mask such as a depth rim.";
    builder.function(
        LIGHT_RIM,
        function(
            vec![
                number("rim"),
                space("normal"),
                space("color").with_default(Constant::vec3(1.0, 1.0, 1.0)),
                number("strength").with_default(Constant::scalar(1.0)),
            ],
            vec![space("color")],
            doc,
        ),
        |b| {
            let rim = b.function_input("rim");
            let normal = b.function_input("normal");
            let color = b.function_input("color");
            let strength = b.function_input("strength");
            let toward = b.input(Input::LightDirection);
            let facing = b.dot(&normal, toward);
            let side = kit::value(
                b,
                "npr.light_side",
                "value",
                [
                    ("facing", facing.into()),
                    ("low", 0.0.into()),
                    ("high", 0.5.into()),
                ],
            );
            let light = kit::node(b, "light.sample", [("normal", normal.into())]);
            let radiance = b.out(light, "color");
            let falloff = b.out(light, "attenuation");
            let mask = b.multiply(rim, side);
            let mask = b.multiply(mask, strength);
            let lit = b.multiply(color, mask);
            let lit = b.multiply(lit, radiance);
            let lit = b.multiply(lit, falloff);
            let lit = b.multiply(lit, 1.0 / PI);
            b.function_output("color", lit);
        },
    );
}

pub(super) fn light_rim<const N: usize>(
    builder: &mut Builder<'_>,
    inputs: [(&str, Arg); N],
) -> Wire {
    let node = builder.call(LIGHT_RIM, inputs);
    builder.out(node, "color")
}

pub(super) fn define_ambient(builder: &mut Builder<'_>) {
    let doc = "Ambient light on albedo: the scene's indirect diffuse at normal (along the bent normal, occluded, traced when the frame traces light) blended by flatten towards the even average of the sky above and below times occlusion, the flat ambient of cel looks.";
    builder.function(
        AMBIENT,
        function(
            vec![
                space("albedo"),
                space("normal"),
                number("occlusion").with_default(Constant::scalar(1.0)),
                number("flatten").with_default(Constant::scalar(0.0)),
            ],
            vec![space("color")],
            doc,
        ),
        |b| {
            let albedo = b.function_input("albedo");
            let normal = b.function_input("normal");
            let occlusion = b.function_input("occlusion");
            let flatten = b.function_input("flatten");
            let indirect = kit::value(
                b,
                "light.indirect",
                "diffuse",
                [
                    ("normal", (&normal).into()),
                    ("albedo", (&albedo).into()),
                    ("occlusion", (&occlusion).into()),
                ],
            );
            let even = kit::value(
                b,
                "light.environment",
                "equalized",
                [("normal", normal.into()), ("equalization", 1.0.into())],
            );
            let even = b.multiply(even, occlusion);
            let blended = b.lerp(indirect, even, flatten);
            let lit = b.multiply(albedo, blended);
            b.function_output("color", lit);
        },
    );
}

pub(super) fn ambient<const N: usize>(builder: &mut Builder<'_>, inputs: [(&str, Arg); N]) -> Wire {
    let node = builder.call(AMBIENT, inputs);
    builder.out(node, "color")
}

pub(super) struct Gloss<'w> {
    pub(super) albedo: &'w Wire,
    pub(super) metalness: Arg,
    pub(super) roughness: Arg,
    pub(super) weight: Arg,
    pub(super) normal: Arg,
    pub(super) occlusion: Arg,
}

pub(super) fn specular_layer(builder: &mut Builder<'_>, gloss: Gloss<'_>) -> Wire {
    builder.frame("Physically based specular", |b| {
        let base = b.multiply(gloss.albedo, gloss.metalness.clone());
        let surface = kit::node(
            b,
            "bsdf.principled",
            [
                ("base_color", base.into()),
                ("metalness", gloss.metalness),
                ("roughness", gloss.roughness),
                ("specular_weight", gloss.weight),
                ("normal", gloss.normal),
                ("occlusion", gloss.occlusion),
            ],
        );
        b.out(surface, "shader")
    })
}

pub(super) fn depth_rim(builder: &mut Builder<'_>, normal: &Wire, width: impl Into<Arg>) -> Wire {
    kit::value(
        builder,
        "npr.depth_rim",
        "mask",
        [
            ("normal", normal.into()),
            ("width", width.into()),
            ("scaling", 1.0.into()),
        ],
    )
}
