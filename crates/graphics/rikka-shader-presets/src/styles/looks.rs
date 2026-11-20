use std::f32::consts::{PI, TAU};

use rikka_shader_graph::{
    Arg, Builder, Constant, Error, Function, FunctionPort, Graph, GraphKind, Input, Library,
    NodeKind, Op, ValueType, Wire,
};

use super::blocks;
use super::kit;
use crate::{Family, Preset};

pub(super) fn presets() -> Vec<Preset> {
    let stylized = |key, title, summary, build| Preset {
        key,
        title,
        summary,
        family: Family::Stylized,
        build,
    };
    vec![
        stylized(
            "painterly",
            "Painterly",
            "Soft painted bands whose edges wander with brush noise, cool hue-shifted shadows and canvas grain.",
            painterly,
        ),
        stylized(
            "halftone",
            "Halftone",
            "Comic print: flat colour in the light and screen-space ink dots in the shadow, the dots growing as the shadow deepens.",
            halftone,
        ),
        stylized(
            "hatching",
            "Hatching",
            "Pen crosshatching from procedural tonal art maps: more and wider line layers, at new angles, as the tone darkens.",
            hatching,
        ),
        stylized(
            "gooch",
            "Gooch",
            "Technical illustration: cool to warm shading by the light's side instead of dark to light, with a white highlight and outline.",
            gooch,
        ),
        stylized(
            "flat",
            "Flat",
            "Unlit flat colour with an outline, for 2D-like looks.",
            flat,
        ),
        stylized(
            "sketch",
            "Pencil sketch",
            "Graphite on paper: wobbly pencil strokes by tone, contours and creases from curvature, and paper grain.",
            sketch,
        ),
        stylized(
            "glow",
            "Hologram glow",
            "Emissive energy: a fresnel-bright glow with scrolling scanlines and flicker that feeds bloom.",
            glow,
        ),
    ]
}

const HALFTONE: &str = "Halftone dots";
const HATCH: &str = "Hatch lines";

fn number(name: &str) -> FunctionPort {
    FunctionPort::new(name, ValueType::Float)
}

fn plane(name: &str) -> FunctionPort {
    FunctionPort::new(name, ValueType::Float2)
}

fn rotate(builder: &mut Builder<'_>, at: impl Into<Arg>, angle: impl Into<Arg>) -> Wire {
    let node = builder.with(
        NodeKind::Op(Op::Rotate),
        [
            ("uv", at.into()),
            ("center", [0.0, 0.0].into()),
            ("angle", angle.into()),
        ],
    );
    builder.out(node, "value")
}

fn define_halftone(builder: &mut Builder<'_>) {
    let doc = "A screen of round ink dots: cells size pixels wide on a grid turned by angle radians, each dot covering density of its cell (0 none, 1 touching neighbours), with an antialiased edge. ink is 1 inside a dot.";
    builder.function(
        HALFTONE,
        Function {
            inputs: vec![
                plane("screen"),
                number("size").with_default(Constant::scalar(6.0)),
                number("angle").with_default(Constant::scalar(PI / 4.0)),
                number("density"),
            ],
            outputs: vec![number("ink")],
            doc: doc.to_owned(),
            body: Default::default(),
        },
        |b| {
            let screen = b.function_input("screen");
            let size = b.function_input("size");
            let angle = b.function_input("angle");
            let density = b.function_input("density");
            let size = kit::maximum(b, size, 1.0);
            let cells = kit::divide(b, screen, &size);
            let turned = rotate(b, cells, angle);
            let inside = b.op(Op::Fraction, [turned.into()]);
            let centred = b.subtract(inside, [0.5, 0.5]);
            let distance = b.op(Op::Length, [centred.into()]);
            let covered = b.saturate(density);
            let area = b.multiply(covered, 1.0 / PI);
            let radius = b.op(Op::SquareRoot, [area.into()]);
            let blur = kit::divide(b, 0.75, &size);
            let inner = b.subtract(&radius, &blur);
            let outer = b.add(&radius, &blur);
            let edge = b.smoothstep(inner, outer, distance);
            let dot = kit::one_minus(b, edge);
            let present = kit::divide(b, radius, blur);
            let present = b.saturate(present);
            let ink = b.multiply(dot, present);
            b.function_output("ink", ink);
        },
    );
}

fn define_hatch(builder: &mut Builder<'_>) {
    let doc = "Parallel pen lines: coord turned by angle radians, one line per unit across, each width of the gap between lines thick (0 none, 1 solid) with an antialiased edge of blur units. ink is 1 on a line.";
    builder.function(
        HATCH,
        Function {
            inputs: vec![
                plane("coord"),
                number("angle"),
                number("width"),
                number("blur").with_default(Constant::scalar(0.08)),
            ],
            outputs: vec![number("ink")],
            doc: doc.to_owned(),
            body: Default::default(),
        },
        |b| {
            let coord = b.function_input("coord");
            let angle = b.function_input("angle");
            let width = b.function_input("width");
            let blur = b.function_input("blur");
            let turned = rotate(b, coord, angle);
            let across = kit::channel(b, &turned, "y");
            let inside = b.op(Op::Fraction, [across.into()]);
            let offset = b.subtract(inside, 0.5);
            let distance = b.op(Op::Absolute, [offset.into()]);
            let distance = b.multiply(distance, 2.0);
            let width = b.saturate(width);
            let inner = b.subtract(&width, &blur);
            let outer = b.add(&width, &blur);
            let edge = b.smoothstep(inner, outer, distance);
            let line = kit::one_minus(b, edge);
            let present = kit::divide(b, width, blur);
            let present = b.saturate(present);
            let ink = b.multiply(line, present);
            b.function_output("ink", ink);
        },
    );
}

fn halftone_dots<const N: usize>(builder: &mut Builder<'_>, inputs: [(&str, Arg); N]) -> Wire {
    let node = builder.call(HALFTONE, inputs);
    builder.out(node, "ink")
}

fn hatch_lines<const N: usize>(builder: &mut Builder<'_>, inputs: [(&str, Arg); N]) -> Wire {
    let node = builder.call(HATCH, inputs);
    builder.out(node, "ink")
}

fn tone(builder: &mut Builder<'_>, normal: impl Into<Arg>) -> Wire {
    builder.frame("Light tone", |b| {
        let diffuse = b.shader(
            NodeKind::Extern("bsdf.diffuse".to_owned()),
            [("color", [1.0, 1.0, 1.0].into()), ("normal", normal.into())],
        );
        let seen = kit::shader_color(b, &diffuse);
        b.op(Op::Luminance, [seen.into()])
    })
}

fn finish(
    mut builder: Builder<'_>,
    surface: Wire,
    alpha: Wire,
    albedo: &Wire,
    outline: f32,
) -> Result<Graph, Error> {
    let line = super::outline(&mut builder, albedo, outline, [0.05, 0.05, 0.07]);
    builder.output([
        ("surface", surface.into()),
        ("alpha", alpha.into()),
        ("outline", line.shader.into()),
        ("outline_width", line.width.into()),
    ]);
    builder.finish()
}

fn painterly(library: &Library) -> Result<Graph, Error> {
    let mut b = Builder::new(GraphKind::Material, library);
    blocks::define_brush(&mut b);
    blocks::define_lit(&mut b);
    blocks::define_ambient(&mut b);
    let albedo = super::albedo(&mut b, [1.0, 1.0, 1.0, 1.0]);
    let normal = super::mapped_normal(&mut b);
    let canvas = b.frame("Canvas grain", |b| {
        let strength = b.parameter(
            kit::unit("canvas_strength", 0.12)
                .with_group("canvas")
                .with_label("Grain"),
        );
        let scale = b.parameter(
            kit::amount("canvas_scale", 120.0, 10.0, 400.0)
                .with_group("canvas")
                .with_label("Weave scale"),
        );
        let at = kit::uv(&mut *b);
        let weave = b.with(
            NodeKind::Op(Op::Checker),
            [("position", at.clone().into()), ("scale", (&scale).into())],
        );
        let weave = b.out(weave, "fac");
        let fibre = b.op(Op::ValueNoise, [at.into(), scale.into()]);
        let grain = b.lerp(weave, fibre, 0.7);
        let grain = b.subtract(grain, 0.5);
        let grain = b.multiply(grain, strength);
        let grain = kit::one_minus(b, grain);
        b.multiply(&albedo.rgb, grain)
    });
    let palette = b.frame("Shadow palette", |b| {
        let shade = b.parameter(super::shade_color([0.55, 0.52, 0.75]));
        let hue = b.parameter(
            kit::amount("shadow_hue_shift", 0.04, -0.2, 0.2)
                .with_group("shade")
                .with_label("Hue shift"),
        );
        let saturation = b.parameter(
            kit::amount("shadow_saturation", 1.25, 0.0, 2.0)
                .with_group("shade")
                .with_label("Saturation"),
        );
        let turn = b.add(hue, 0.5);
        let shifted = b.with(
            NodeKind::Op(Op::HueSaturation),
            [
                ("color", (&canvas).into()),
                ("hue", turn.into()),
                ("saturation", saturation.into()),
            ],
        );
        let shifted = b.out(shifted, "value");
        let deep = b.multiply(shifted, shade);
        let middle = b.lerp(&canvas, &deep, 0.45);
        (middle, deep)
    });
    let (middle, deep) = palette;
    let painted = b.frame("Painted bands (per light)", |b| {
        let low = b.parameter(
            kit::unit("deep_edge", 0.3)
                .with_group("bands")
                .with_label("Deep edge"),
        );
        let high = b.parameter(
            kit::unit("light_edge", 0.55)
                .with_group("bands")
                .with_label("Light edge"),
        );
        let softness = b.parameter(
            kit::amount("band_softness", 0.12, 0.0, 0.5)
                .with_group("bands")
                .with_label("Softness"),
        );
        let amount = b.parameter(
            kit::amount("brush_amount", 0.25, 0.0, 1.0)
                .with_group("brush")
                .with_label("Amount"),
        );
        let scale = b.parameter(
            kit::amount("brush_scale", 10.0, 1.0, 64.0)
                .with_group("brush")
                .with_label("Scale"),
        );
        let stretch = b.parameter(
            kit::amount("brush_stretch", 5.0, 1.0, 16.0)
                .with_group("brush")
                .with_label("Stretch"),
        );
        let wrapped = kit::value(b, "npr.wrap", "value", [("normal", (&normal).into())]);
        let shadow = kit::value(b, "light.shadow", "visibility", []);
        let value = b.multiply(wrapped, shadow);
        let at = kit::uv(b);
        let toward = b.input(Input::LightDirection);
        let jitter = blocks::brush(
            b,
            [
                ("uv", at.into()),
                ("light", toward.into()),
                ("amount", amount.into()),
                ("scale", scale.into()),
                ("stretch", stretch.into()),
            ],
        );
        let value = b.add(value, jitter);
        let colour = kit::value(
            b,
            "npr.bands",
            "color",
            [
                ("value", value.into()),
                ("low", low.into()),
                ("high", high.into()),
                ("smoothness", softness.into()),
                ("lit_color", (&canvas).into()),
                ("shallow_color", (&middle).into()),
                ("deep_color", (&deep).into()),
            ],
        );
        blocks::lit(b, colour, [1.0, 1.0, 1.0])
    });
    let ambient = b.frame("Ambient", |b| {
        blocks::ambient(
            b,
            [
                ("albedo", (&deep).into()),
                ("normal", (&normal).into()),
                ("flatten", 0.8.into()),
            ],
        )
    });
    let surface = kit::lighting(&mut b, &painted, &ambient);
    finish(b, surface, albedo.alpha, &deep, 0.001)
}

fn halftone(library: &Library) -> Result<Graph, Error> {
    let mut b = Builder::new(GraphKind::Material, library);
    define_halftone(&mut b);
    let albedo = super::albedo(&mut b, [1.0, 1.0, 1.0, 1.0]);
    let normal = super::mapped_normal(&mut b);
    let tone = tone(&mut b, &normal);
    let printed = b.frame("Comic print", |b| {
        let shade = b.parameter(super::shade_color([0.85, 0.85, 0.92]));
        let ink = b.parameter(
            kit::color("ink_color", [0.06, 0.06, 0.1])
                .with_group("halftone")
                .with_label("Ink"),
        );
        let threshold = b.parameter(
            kit::amount("shadow_threshold", 0.3, 0.0, 1.0)
                .with_group("halftone")
                .with_label("Shadow threshold"),
        );
        let size = b.parameter(
            kit::amount("dot_size", 6.0, 2.0, 32.0)
                .with_group("halftone")
                .with_label("Dot size (pixels)"),
        );
        let angle = b.parameter(
            rikka_shader_graph::ParameterDecl::float("dot_angle", PI / 4.0)
                .with_group("halftone")
                .with_label("Screen angle")
                .with_hint(rikka_slang::Hint::Angle),
        );
        let floor = kit::maximum(b, &threshold, 1e-3);
        let missing = b.subtract(&threshold, &tone);
        let density = kit::divide(b, missing, floor);
        let density = b.saturate(density);
        let screen = b.input(Input::ScreenPosition);
        let dots = halftone_dots(
            b,
            [
                ("screen", screen.into()),
                ("size", size.into()),
                ("angle", angle.into()),
                ("density", density.into()),
            ],
        );
        let low = b.subtract(&threshold, 0.01);
        let high = b.add(&threshold, 0.01);
        let lit = b.smoothstep(low, high, &tone);
        let shaded = b.multiply(&albedo.rgb, shade);
        let flat = b.lerp(shaded, &albedo.rgb, lit);
        let printed = b.lerp(flat, ink, dots);
        kit::emission(b, printed)
    });
    finish(b, printed, albedo.alpha, &albedo.rgb, 0.002)
}

fn hatching(library: &Library) -> Result<Graph, Error> {
    let mut b = Builder::new(GraphKind::Material, library);
    define_hatch(&mut b);
    let albedo = super::albedo(&mut b, [1.0, 1.0, 1.0, 1.0]);
    let normal = super::mapped_normal(&mut b);
    let tone = tone(&mut b, &normal);
    let coord = b.frame("Stroke space", |b| {
        let space = b.parameter(
            kit::options("hatch_space", &["uv", "screen"], 0)
                .with_group("hatching")
                .with_label("Lines follow"),
        );
        let density = b.parameter(
            kit::amount("hatch_density", 80.0, 4.0, 400.0)
                .with_group("hatching")
                .with_label("Lines per uv unit"),
        );
        let spacing = b.parameter(
            kit::amount("hatch_spacing", 5.0, 2.0, 32.0)
                .with_group("hatching")
                .with_label("Line spacing (pixels)"),
        );
        let at = kit::uv(b);
        let at = b.multiply(at, density);
        let screen = b.input(Input::ScreenPosition);
        let screen = kit::divide(b, screen, spacing);
        b.lerp(at, screen, space)
    });
    let drawn = b.frame("Tonal art map", |b| {
        let paper = b.parameter(
            kit::unit("paper_tint", 1.0)
                .with_group("hatching")
                .with_label("Albedo on paper"),
        );
        let ink = b.parameter(
            kit::color("ink_color", [0.08, 0.08, 0.12])
                .with_group("hatching")
                .with_label("Ink"),
        );
        let lightest = b.parameter(
            kit::unit("hatch_start", 0.8)
                .with_group("hatching")
                .with_label("First layer below tone"),
        );
        let weight = b.parameter(
            kit::unit("line_weight", 0.45)
                .with_group("hatching")
                .with_label("Line weight"),
        );
        let layers = [
            (1.0, PI / 4.0),
            (0.65, -PI / 4.0),
            (0.4, 0.0),
            (0.2, PI / 2.0),
        ];
        let mut total: Option<Wire> = None;
        for (share, angle) in layers {
            let start = b.multiply(&lightest, share);
            let missing = b.subtract(&start, &tone);
            let span = b.multiply(&start, 0.6);
            let span = kit::maximum(b, span, 1e-3);
            let depth = kit::divide(b, missing, span);
            let depth = b.saturate(depth);
            let width = b.multiply(depth, &weight);
            let lines = hatch_lines(
                b,
                [
                    ("coord", (&coord).into()),
                    ("angle", angle.into()),
                    ("width", width.into()),
                ],
            );
            total = Some(match total {
                Some(sum) => kit::maximum(b, sum, lines),
                None => lines,
            });
        }
        let lines = total.expect("four layers");
        let sheet = b.lerp([1.0, 1.0, 1.0], &albedo.rgb, paper);
        let drawn = b.lerp(sheet, ink, lines);
        kit::emission(b, drawn)
    });
    finish(b, drawn, albedo.alpha, &albedo.rgb, 0.0015)
}

fn gooch(library: &Library) -> Result<Graph, Error> {
    let mut b = Builder::new(GraphKind::Material, library);
    blocks::define_lit(&mut b);
    let albedo = super::albedo(&mut b, [0.8, 0.8, 0.8, 1.0]);
    let normal = super::mapped_normal(&mut b);
    let (cool, warm) = b.frame("Cool and warm", |b| {
        let cool = b.parameter(
            kit::color("cool_color", [0.0, 0.0, 0.55])
                .with_group("gooch")
                .with_label("Cool"),
        );
        let warm = b.parameter(
            kit::color("warm_color", [0.35, 0.3, 0.0])
                .with_group("gooch")
                .with_label("Warm"),
        );
        let alpha = b.parameter(
            kit::unit("cool_albedo", 0.25)
                .with_group("gooch")
                .with_label("Albedo in cool"),
        );
        let beta = b.parameter(
            kit::unit("warm_albedo", 0.5)
                .with_group("gooch")
                .with_label("Albedo in warm"),
        );
        let cool_part = b.multiply(&albedo.rgb, alpha);
        let warm_part = b.multiply(&albedo.rgb, beta);
        (b.add(cool, cool_part), b.add(warm, warm_part))
    });
    let shaded = b.frame("Gooch shading (per light)", |b| {
        let strength = b.parameter(
            kit::amount("highlight_strength", 0.6, 0.0, 4.0)
                .with_group("gooch")
                .with_label("Highlight"),
        );
        let power = b.parameter(
            kit::amount("highlight_power", 32.0, 1.0, 256.0)
                .with_group("gooch")
                .with_label("Highlight sharpness"),
        );
        let toward = b.input(Input::LightDirection);
        let facing = b.dot(&normal, &toward);
        let shadow = kit::value(b, "light.shadow", "visibility", []);
        let side = b.add(facing, 1.0);
        let side = b.multiply(side, 0.5);
        let side = b.multiply(side, &shadow);
        let tone = b.lerp(&cool, &warm, side);
        let incoming = b.op(Op::Negate, [toward.into()]);
        let mirrored = b.op(Op::Reflect, [incoming.into(), (&normal).into()]);
        let view = kit::view(b);
        let glint = b.dot(mirrored, view);
        let glint = b.saturate(glint);
        let glint = b.op(Op::Power, [glint.into(), power.into()]);
        let glint = b.multiply(glint, strength);
        let glint = b.multiply(glint, shadow);
        let tone = b.add(tone, glint);
        blocks::lit(b, [1.0, 1.0, 1.0], tone)
    });
    let dark = kit::combine(&mut b, [0.0.into(), 0.0.into(), 0.0.into()]);
    let surface = kit::lighting(&mut b, &shaded, &dark);
    finish(b, surface, albedo.alpha, &albedo.rgb, 0.0015)
}

fn flat(library: &Library) -> Result<Graph, Error> {
    let mut b = Builder::new(GraphKind::Material, library);
    let albedo = super::albedo(&mut b, [1.0, 1.0, 1.0, 1.0]);
    let glow = super::emitted(&mut b);
    let surface = b.frame("Unlit colour", |b| {
        let colour = b.add(&albedo.rgb, glow);
        kit::emission(b, colour)
    });
    finish(b, surface, albedo.alpha, &albedo.rgb, 0.0015)
}

fn sketch(library: &Library) -> Result<Graph, Error> {
    let mut b = Builder::new(GraphKind::Material, library);
    define_hatch(&mut b);
    let albedo = super::albedo(&mut b, [1.0, 1.0, 1.0, 1.0]);
    let normal = super::mapped_normal(&mut b);
    let tone = tone(&mut b, &normal);
    let lines = b.frame("Contours and creases", |b| {
        let contour = b.parameter(
            kit::amount("contour_power", 6.0, 1.0, 32.0)
                .with_group("pencil")
                .with_label("Contour tightness"),
        );
        let crease = b.parameter(
            kit::amount("crease_strength", 0.02, 0.0, 0.2)
                .with_group("pencil")
                .with_label("Crease strength (metres)"),
        );
        let view = kit::view(b);
        let rim = kit::value(
            b,
            "npr.fresnel_rim",
            "value",
            [
                ("normal", (&normal).into()),
                ("view", view.into()),
                ("power", contour.into()),
            ],
        );
        let bend = kit::value(
            b,
            "vector.curvature",
            "magnitude",
            [("normal", (&normal).into())],
        );
        let bend = b.multiply(bend, crease);
        let bend = b.smoothstep(0.3, 1.0, bend);
        kit::maximum(b, rim, bend)
    });
    let drawn = b.frame("Pencil strokes", |b| {
        let paper = b.parameter(
            kit::color("paper_color", [0.96, 0.95, 0.91])
                .with_group("pencil")
                .with_label("Paper"),
        );
        let graphite = b.parameter(
            kit::color("graphite_color", [0.22, 0.22, 0.26])
                .with_group("pencil")
                .with_label("Graphite"),
        );
        let spacing = b.parameter(
            kit::amount("stroke_spacing", 4.0, 2.0, 16.0)
                .with_group("pencil")
                .with_label("Stroke spacing (pixels)"),
        );
        let angle = b.parameter(
            rikka_shader_graph::ParameterDecl::float("stroke_angle", 0.6)
                .with_group("pencil")
                .with_label("Stroke angle")
                .with_hint(rikka_slang::Hint::Angle),
        );
        let influence = b.parameter(
            kit::unit("albedo_influence", 0.4)
                .with_group("pencil")
                .with_label("Albedo in tone"),
        );
        let wobble = b.parameter(
            kit::amount("stroke_wobble", 0.6, 0.0, 3.0)
                .with_group("pencil")
                .with_label("Wobble"),
        );
        let brightness = b.op(Op::Luminance, [(&albedo.rgb).into()]);
        let weight = b.lerp(1.0, brightness, influence);
        let value = b.multiply(&tone, weight);
        let screen = b.input(Input::ScreenPosition);
        let cells = kit::divide(b, &screen, spacing);
        let drift = b.op(Op::GradientNoise, [(&screen).into(), (1.0 / 40.0).into()]);
        let drift = b.subtract(drift, 0.5);
        let drift = b.multiply(drift, wobble);
        let coord = b.add(cells, drift);
        let grain = b.op(Op::ValueNoise, [screen.into(), 0.7.into()]);
        let grain = b.lerp(0.55, 1.0, grain);
        let mut total: Option<Wire> = None;
        for (start, turn) in [(0.7, 0.0), (0.4, 0.9)] {
            let missing = b.subtract(start, &value);
            let depth = kit::divide(b, missing, start);
            let depth = b.saturate(depth);
            let width = b.multiply(depth, 0.55);
            let tilt = b.add(&angle, turn);
            let stroke = hatch_lines(
                b,
                [
                    ("coord", (&coord).into()),
                    ("angle", tilt.into()),
                    ("width", width.into()),
                    ("blur", 0.2.into()),
                ],
            );
            total = Some(match total {
                Some(sum) => kit::maximum(b, sum, stroke),
                None => stroke,
            });
        }
        let strokes = total.expect("two layers");
        let strokes = b.multiply(strokes, &grain);
        let edges = b.multiply(&lines, grain);
        let ink = kit::maximum(b, strokes, edges);
        let ink = b.saturate(ink);
        let drawn = b.lerp(paper, graphite, ink);
        kit::emission(b, drawn)
    });
    finish(b, drawn, albedo.alpha, &albedo.rgb, 0.0)
}

fn glow(library: &Library) -> Result<Graph, Error> {
    let mut b = Builder::new(GraphKind::Material, library);
    let albedo = super::albedo(&mut b, [1.0, 1.0, 1.0, 1.0]);
    let normal = super::mapped_normal(&mut b);
    let (energy, alpha) = b.frame("Energy", |b| {
        let color = b.parameter(
            kit::color("glow_color", [0.2, 0.8, 1.0])
                .with_group("glow")
                .with_label("Colour"),
        );
        let strength = b.parameter(
            kit::amount("glow_strength", 4.0, 0.0, 32.0)
                .with_group("glow")
                .with_label("Strength"),
        );
        let fill = b.parameter(
            kit::unit("glow_fill", 0.15)
                .with_group("glow")
                .with_label("Fill"),
        );
        let power = b.parameter(
            kit::amount("rim_power", 2.5, 0.5, 8.0)
                .with_group("glow")
                .with_label("Rim power"),
        );
        let transparency = b.parameter(
            kit::unit("transparency", 0.0)
                .with_group("glow")
                .with_label("See-through"),
        );
        let view = kit::view(b);
        let rim = kit::value(
            b,
            "npr.fresnel_rim",
            "value",
            [
                ("normal", (&normal).into()),
                ("view", view.into()),
                ("power", power.into()),
            ],
        );
        let body = b.add(fill, rim);
        let body = b.saturate(body);
        let alpha = b.lerp(1.0, &body, transparency);
        let alpha = b.multiply(alpha, &albedo.alpha);
        let tint = b.multiply(color, &albedo.rgb);
        let energy = b.multiply(tint, body);
        (b.multiply(energy, strength), alpha)
    });
    let pulsed = b.frame("Scanlines and flicker", |b| {
        let density = b.parameter(
            kit::amount("scanline_density", 40.0, 1.0, 400.0)
                .with_group("scanlines")
                .with_label("Lines per metre"),
        );
        let speed = b.parameter(
            kit::amount("scanline_speed", 0.5, -4.0, 4.0)
                .with_group("scanlines")
                .with_label("Speed"),
        );
        let depth = b.parameter(
            kit::unit("scanline_strength", 0.5)
                .with_group("scanlines")
                .with_label("Strength"),
        );
        let flicker = b.parameter(
            kit::unit("flicker", 0.1)
                .with_group("scanlines")
                .with_label("Flicker"),
        );
        let position = kit::position(b);
        let height = kit::channel(b, &position, "y");
        let time = b.input(Input::Time);
        let rows = b.multiply(height, density);
        let drift = b.multiply(&time, speed);
        let phase = b.subtract(rows, drift);
        let phase = b.multiply(phase, TAU);
        let wave = b.op(Op::Sine, [phase.into()]);
        let wave = b.multiply(wave, 0.5);
        let wave = b.add(wave, 0.5);
        let wave = b.op(Op::Power, [wave.into(), 4.0.into()]);
        let lines = b.lerp(1.0, wave, depth);
        let beat = b.multiply(time, 12.0);
        let noise = b.op(Op::ValueNoise, [beat.into(), 1.0.into()]);
        let dip = b.multiply(noise, flicker);
        let steady = kit::one_minus(b, dip);
        let pulse = b.multiply(lines, steady);
        b.multiply(&energy, pulse)
    });
    let glow = super::emitted(&mut b);
    let total = b.add(pulsed, glow);
    let surface = kit::emission(&mut b, total);
    b.output([("surface", surface.into()), ("alpha", alpha.into())]);
    b.finish()
}
