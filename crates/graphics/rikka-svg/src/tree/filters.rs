use std::collections::HashSet;

use rikka_types::{Color, Rect};
use rikka_vector::BlendMode;

use crate::parse::dom::{Dom, NodeId, Tag};
use crate::parse::values::{AspectRatio, Length, length, number, numbers};
use crate::tree::servers::{measure, units};
use crate::tree::style::{Axis, Context, Viewport};
use crate::tree::{Group, PictureContent, Units};

pub(crate) const MAX_PRIMITIVES: usize = 256;
const MAX_LIVE_RESULTS: usize = 16;
const MAX_KERNEL_CELLS: usize = 1024;
const MAX_OCTAVES: f32 = 10.0;
const NOTHING: Kind = Kind::Flood {
    color: Color::rgba(0.0, 0.0, 0.0, 0.0),
};

#[derive(Clone, Debug)]
pub(crate) struct Filter {
    pub(crate) units: Units,
    pub(crate) primitive_units: Units,
    pub(crate) rect: Rect,
    pub(crate) primitives: Vec<Primitive>,
}

#[derive(Clone, Debug)]
pub(crate) struct Primitive {
    pub(crate) kind: Kind,
    pub(crate) result: String,
    pub(crate) region: [Option<f32>; 4],
    pub(crate) linear: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Input {
    Source,
    SourceAlpha,
    Previous,
    Named(String),
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Operator {
    Over,
    In,
    Out,
    Atop,
    Xor,
    Arithmetic([f32; 4]),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Transfer {
    Identity,
    Table(Vec<f32>),
    Discrete(Vec<f32>),
    Linear {
        slope: f32,
        intercept: f32,
    },
    Gamma {
        amplitude: f32,
        exponent: f32,
        offset: f32,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum Kind {
    Blur {
        input: Input,
        deviation: (f32, f32),
    },
    Offset {
        input: Input,
        dx: f32,
        dy: f32,
    },
    Flood {
        color: Color,
    },
    Merge {
        inputs: Vec<Input>,
    },
    Composite {
        input: Input,
        second: Input,
        operator: Operator,
    },
    Blend {
        input: Input,
        second: Input,
        mode: BlendMode,
    },
    ColorMatrix {
        input: Input,
        matrix: [f32; 20],
    },
    Transfer {
        input: Input,
        functions: [Transfer; 4],
    },
    DropShadow {
        input: Input,
        dx: f32,
        dy: f32,
        deviation: (f32, f32),
        color: Color,
    },
    Morphology {
        input: Input,
        radius: (f32, f32),
        dilate: bool,
    },
    Tile {
        input: Input,
    },
    Image {
        content: Option<ImageContent>,
    },
    Turbulence {
        frequency: (f32, f32),
        octaves: u32,
        seed: i32,
        stitch: bool,
        fractal: bool,
    },
    Convolve {
        input: Input,
        kernel: Kernel,
    },
    Displace {
        input: Input,
        map: Input,
        scale: f32,
        channels: (usize, usize),
    },
    Lighting {
        input: Input,
        lighting: Lighting,
    },
    Passthrough {
        input: Input,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum ImageContent {
    Element(Group),
    Picture {
        content: PictureContent,
        aspect: AspectRatio,
        smooth: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edge {
    Duplicate,
    Wrap,
    Transparent,
}

#[derive(Clone, Debug)]
pub(crate) struct Kernel {
    pub(crate) columns: usize,
    pub(crate) rows: usize,
    pub(crate) values: Vec<f32>,
    pub(crate) target: (usize, usize),
    pub(crate) divisor: f32,
    pub(crate) bias: f32,
    pub(crate) edge: Edge,
    pub(crate) preserve_alpha: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Light {
    Distant {
        azimuth: f32,
        elevation: f32,
    },
    Point {
        position: [f32; 3],
    },
    Spot {
        position: [f32; 3],
        target: [f32; 3],
        exponent: f32,
        cone: Option<f32>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Reflection {
    Diffuse { constant: f32 },
    Specular { constant: f32, exponent: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lighting {
    pub(crate) light: Light,
    pub(crate) reflection: Reflection,
    pub(crate) surface_scale: f32,
    pub(crate) color: Color,
}

impl Kind {
    fn inputs(&self) -> Vec<&Input> {
        match self {
            Kind::Flood { .. } | Kind::Image { .. } | Kind::Turbulence { .. } => Vec::new(),
            Kind::Merge { inputs } => inputs.iter().collect(),
            Kind::Composite { input, second, .. } | Kind::Blend { input, second, .. } => {
                vec![input, second]
            }
            Kind::Displace { input, map, .. } => vec![input, map],
            Kind::Blur { input, .. }
            | Kind::Offset { input, .. }
            | Kind::ColorMatrix { input, .. }
            | Kind::Transfer { input, .. }
            | Kind::DropShadow { input, .. }
            | Kind::Morphology { input, .. }
            | Kind::Tile { input }
            | Kind::Convolve { input, .. }
            | Kind::Lighting { input, .. }
            | Kind::Passthrough { input } => vec![input],
        }
    }
}

const IDENTITY: [f32; 20] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    0.0,
];

pub(crate) fn blend_mode(name: &str) -> BlendMode {
    match name {
        "multiply" => BlendMode::Multiply,
        "screen" => BlendMode::Screen,
        "overlay" => BlendMode::Overlay,
        "darken" => BlendMode::Darken,
        "lighten" => BlendMode::Lighten,
        "color-dodge" => BlendMode::ColorDodge,
        "color-burn" => BlendMode::ColorBurn,
        "hard-light" => BlendMode::HardLight,
        "soft-light" => BlendMode::SoftLight,
        "difference" => BlendMode::Difference,
        "exclusion" => BlendMode::Exclusion,
        "hue" => BlendMode::HslHue,
        "saturation" => BlendMode::HslSaturation,
        "color" => BlendMode::HslColor,
        "luminosity" => BlendMode::HslLuminosity,
        _ => BlendMode::SourceOver,
    }
}

pub(crate) fn filter(
    dom: &Dom,
    node: NodeId,
    ancestors: &[NodeId],
    viewport: Viewport,
    image: &mut dyn FnMut(NodeId) -> Option<ImageContent>,
) -> Option<Filter> {
    let own = [node];
    let context = Context {
        dom,
        chain: &own,
        viewport,
    };
    let filter_units = units(context.attribute("filterUnits"), Units::BoundingBox);
    let primitive_units = units(context.attribute("primitiveUnits"), Units::UserSpace);
    let side = |name: &str, default: f32, axis: Axis| {
        measure(
            context.attribute(name),
            Length::percent(default),
            filter_units,
            axis,
            &context,
        )
    };
    let rect = Rect::new(
        side("x", -10.0, Axis::Horizontal),
        side("y", -10.0, Axis::Vertical),
        side("width", 120.0, Axis::Horizontal),
        side("height", 120.0, Axis::Vertical),
    );
    let mut chain = ancestors.to_vec();
    chain.push(node);
    let mut primitives = Vec::new();
    for &child in &dom.element(node).children {
        if dom.element(child).tag != Tag::Primitive {
            continue;
        }
        if primitives.len() == MAX_PRIMITIVES {
            return None;
        }
        chain.push(child);
        let context = Context {
            dom,
            chain: &chain,
            viewport,
        };
        let mut kind = primitive_kind(dom, child, &context);
        if let Kind::Image { content } = &mut kind {
            *content = image(child);
        }
        primitives.push(primitive(kind, &context, primitive_units));
        chain.pop();
    }
    if !keep_needed_results(&mut primitives) {
        return None;
    }
    Some(Filter {
        units: filter_units,
        primitive_units,
        rect,
        primitives,
    })
}

fn keep_needed_results(primitives: &mut [Primitive]) -> bool {
    let mut wanted = HashSet::new();
    for primitive in primitives.iter_mut().rev() {
        if !wanted.remove(&primitive.result) {
            primitive.result.clear();
        }
        wanted.extend(
            primitive
                .kind
                .inputs()
                .into_iter()
                .filter_map(|input| match input {
                    Input::Named(name) => Some(name.clone()),
                    _ => None,
                }),
        );
        if wanted.len() > MAX_LIVE_RESULTS {
            return false;
        }
    }
    true
}

fn input(value: Option<&str>) -> Input {
    match value.map(str::trim) {
        None | Some("") => Input::Previous,
        Some("SourceGraphic") => Input::Source,
        Some("SourceAlpha") => Input::SourceAlpha,
        Some("BackgroundImage" | "BackgroundAlpha" | "FillPaint" | "StrokePaint") => Input::Nothing,
        Some(name) => Input::Named(name.to_owned()),
    }
}

fn pair(value: Option<&str>) -> (f32, f32) {
    match numbers(value.unwrap_or_default()).as_slice() {
        [both] => (*both, *both),
        [x, y, ..] => (*x, *y),
        [] => (0.0, 0.0),
    }
}

fn primitive_kind(dom: &Dom, node: NodeId, context: &Context<'_>) -> Kind {
    let attribute = |name: &str| context.attribute(name);
    let value = |name: &str, default: f32| attribute(name).and_then(number).unwrap_or(default);
    let first = input(attribute("in"));
    let flood = || {
        let mut color = context
            .color("flood-color")
            .unwrap_or(Color::rgba(0.0, 0.0, 0.0, 1.0));
        color.a *= context.unit("flood-opacity").unwrap_or(1.0);
        color
    };
    match dom.element(node).name.as_str() {
        "feGaussianBlur" => Kind::Blur {
            input: first,
            deviation: pair(attribute("stdDeviation")),
        },
        "feOffset" => Kind::Offset {
            input: first,
            dx: value("dx", 0.0),
            dy: value("dy", 0.0),
        },
        "feFlood" => Kind::Flood { color: flood() },
        "feMerge" => Kind::Merge {
            inputs: dom
                .element(node)
                .children
                .iter()
                .filter(|child| dom.element(**child).name == "feMergeNode")
                .map(|child| input(dom.attribute(*child, "in")))
                .collect(),
        },
        "feComposite" => Kind::Composite {
            input: first,
            second: input(attribute("in2")),
            operator: match attribute("operator") {
                Some("in") => Operator::In,
                Some("out") => Operator::Out,
                Some("atop") => Operator::Atop,
                Some("xor") => Operator::Xor,
                Some("arithmetic") => Operator::Arithmetic([
                    value("k1", 0.0),
                    value("k2", 0.0),
                    value("k3", 0.0),
                    value("k4", 0.0),
                ]),
                _ => Operator::Over,
            },
        },
        "feBlend" => Kind::Blend {
            input: first,
            second: input(attribute("in2")),
            mode: blend_mode(attribute("mode").unwrap_or("normal")),
        },
        "feColorMatrix" => Kind::ColorMatrix {
            input: first,
            matrix: color_matrix(attribute("type"), attribute("values")),
        },
        "feComponentTransfer" => {
            let function = |name: &str| {
                dom.element(node)
                    .children
                    .iter()
                    .rev()
                    .find(|child| dom.element(**child).name == name)
                    .map_or(Transfer::Identity, |child| transfer(dom, *child))
            };
            Kind::Transfer {
                input: first,
                functions: [
                    function("feFuncR"),
                    function("feFuncG"),
                    function("feFuncB"),
                    function("feFuncA"),
                ],
            }
        }
        "feDropShadow" => Kind::DropShadow {
            input: first,
            dx: value("dx", 2.0),
            dy: value("dy", 2.0),
            deviation: match attribute("stdDeviation") {
                Some(deviation) => pair(Some(deviation)),
                None => (2.0, 2.0),
            },
            color: flood(),
        },
        "feMorphology" => Kind::Morphology {
            input: first,
            radius: pair(attribute("radius")),
            dilate: attribute("operator") == Some("dilate"),
        },
        "feTile" => Kind::Tile { input: first },
        "feImage" => Kind::Image { content: None },
        "feTurbulence" => turbulence(context),
        "feConvolveMatrix" => kernel(context).map_or(NOTHING, |kernel| Kind::Convolve {
            input: first,
            kernel,
        }),
        "feDisplacementMap" => Kind::Displace {
            input: first,
            map: input(attribute("in2")),
            scale: value("scale", 0.0),
            channels: (
                channel(attribute("xChannelSelector")),
                channel(attribute("yChannelSelector")),
            ),
        },
        "feDiffuseLighting" | "feSpecularLighting" => {
            lighting(dom, node, context).map_or(NOTHING, |lighting| Kind::Lighting {
                input: first,
                lighting,
            })
        }
        _ => Kind::Passthrough { input: first },
    }
}

fn turbulence(context: &Context<'_>) -> Kind {
    let frequency = match numbers(context.attribute("baseFrequency").unwrap_or_default()).as_slice()
    {
        [both] if *both >= 0.0 => (*both, *both),
        [x, y] if *x >= 0.0 && *y >= 0.0 => (*x, *y),
        _ => (0.0, 0.0),
    };
    let octaves = context
        .attribute("numOctaves")
        .and_then(number)
        .unwrap_or(1.0)
        .clamp(0.0, MAX_OCTAVES)
        .round() as u32;
    Kind::Turbulence {
        frequency,
        octaves,
        seed: context
            .attribute("seed")
            .and_then(number)
            .unwrap_or(0.0)
            .trunc() as i32,
        stitch: context.attribute("stitchTiles") == Some("stitch"),
        fractal: context.attribute("type") == Some("fractalNoise"),
    }
}

fn kernel(context: &Context<'_>) -> Option<Kernel> {
    let order = numbers(context.attribute("order").unwrap_or("3"));
    let (columns, rows) = match order.as_slice() {
        [both] => (*both, *both),
        [columns, rows] => (*columns, *rows),
        _ => return None,
    };
    if columns < 1.0 || rows < 1.0 || columns.fract() != 0.0 || rows.fract() != 0.0 {
        return None;
    }
    let (columns, rows) = (columns as usize, rows as usize);
    if columns.saturating_mul(rows) > MAX_KERNEL_CELLS {
        return None;
    }
    let values = numbers(context.attribute("kernelMatrix")?);
    if values.len() != columns * rows {
        return None;
    }
    let sum = (values.iter().sum::<f32>() * 1.0e6).round() / 1.0e6;
    let divisor = context
        .attribute("divisor")
        .and_then(number)
        .unwrap_or(if sum == 0.0 { 1.0 } else { sum });
    if divisor == 0.0 {
        return None;
    }
    let target = |name: &str, order: usize| -> Option<usize> {
        let given = context
            .attribute(name)
            .and_then(number)
            .unwrap_or((order / 2) as f32);
        (given >= 0.0 && (given as usize) < order).then_some(given as usize)
    };
    Some(Kernel {
        columns,
        rows,
        values,
        target: (target("targetX", columns)?, target("targetY", rows)?),
        divisor,
        bias: context.attribute("bias").and_then(number).unwrap_or(0.0),
        edge: match context.attribute("edgeMode") {
            Some("wrap") => Edge::Wrap,
            Some("none") => Edge::Transparent,
            _ => Edge::Duplicate,
        },
        preserve_alpha: context.attribute("preserveAlpha") == Some("true"),
    })
}

fn channel(value: Option<&str>) -> usize {
    match value {
        Some("R") => 0,
        Some("G") => 1,
        Some("B") => 2,
        _ => 3,
    }
}

fn lighting(dom: &Dom, node: NodeId, context: &Context<'_>) -> Option<Lighting> {
    let value =
        |name: &str, default: f32| context.attribute(name).and_then(number).unwrap_or(default);
    let source = dom.element(node).children.iter().copied().find(|child| {
        matches!(
            dom.element(*child).name.as_str(),
            "feDistantLight" | "fePointLight" | "feSpotLight"
        )
    })?;
    let at = |name: &str, default: f32| {
        dom.attribute(source, name)
            .and_then(number)
            .unwrap_or(default)
    };
    let light = match dom.element(source).name.as_str() {
        "feDistantLight" => Light::Distant {
            azimuth: at("azimuth", 0.0),
            elevation: at("elevation", 0.0),
        },
        "fePointLight" => Light::Point {
            position: [at("x", 0.0), at("y", 0.0), at("z", 0.0)],
        },
        _ => Light::Spot {
            position: [at("x", 0.0), at("y", 0.0), at("z", 0.0)],
            target: [
                at("pointsAtX", 0.0),
                at("pointsAtY", 0.0),
                at("pointsAtZ", 0.0),
            ],
            exponent: Some(at("specularExponent", 1.0))
                .filter(|exponent| *exponent > 0.0)
                .unwrap_or(1.0),
            cone: dom.attribute(source, "limitingConeAngle").and_then(number),
        },
    };
    let reflection = match dom.element(node).name.as_str() {
        "feDiffuseLighting" => Reflection::Diffuse {
            constant: value("diffuseConstant", 1.0),
        },
        _ => {
            let exponent = value("specularExponent", 1.0);
            if !(1.0..=128.0).contains(&exponent) {
                return None;
            }
            Reflection::Specular {
                constant: value("specularConstant", 1.0),
                exponent,
            }
        }
    };
    let mut color = context
        .color("lighting-color")
        .unwrap_or(Color::rgba(1.0, 1.0, 1.0, 1.0));
    color.a = 1.0;
    Some(Lighting {
        light,
        reflection,
        surface_scale: value("surfaceScale", 1.0),
        color,
    })
}

fn primitive(kind: Kind, context: &Context<'_>, units: Units) -> Primitive {
    let attribute = |name: &str| context.attribute(name);
    let side = |name: &str, axis: Axis| {
        attribute(name)
            .and_then(length)
            .map(|side| measure(attribute(name), side, units, axis, context))
    };
    Primitive {
        kind,
        result: attribute("result").unwrap_or_default().to_owned(),
        region: [
            side("x", Axis::Horizontal),
            side("y", Axis::Vertical),
            side("width", Axis::Horizontal),
            side("height", Axis::Vertical),
        ],
        linear: !context.is("color-interpolation-filters", "sRGB"),
    }
}

fn transfer(dom: &Dom, node: NodeId) -> Transfer {
    let attribute = |name: &str| dom.attribute(node, name);
    let value = |name: &str, default: f32| attribute(name).and_then(number).unwrap_or(default);
    let table = || numbers(attribute("tableValues").unwrap_or_default());
    match attribute("type") {
        Some("table") if !table().is_empty() => Transfer::Table(table()),
        Some("discrete") if !table().is_empty() => Transfer::Discrete(table()),
        Some("linear") => Transfer::Linear {
            slope: value("slope", 1.0),
            intercept: value("intercept", 0.0),
        },
        Some("gamma") => Transfer::Gamma {
            amplitude: value("amplitude", 1.0),
            exponent: value("exponent", 1.0),
            offset: value("offset", 0.0),
        },
        _ => Transfer::Identity,
    }
}

fn color_matrix(kind: Option<&str>, values: Option<&str>) -> [f32; 20] {
    let values = numbers(values.unwrap_or_default());
    match kind {
        Some("saturate") => {
            let s = values.first().copied().unwrap_or(1.0).max(0.0);
            [
                0.213 + 0.787 * s,
                0.715 - 0.715 * s,
                0.072 - 0.072 * s,
                0.0,
                0.0,
                0.213 - 0.213 * s,
                0.715 + 0.285 * s,
                0.072 - 0.072 * s,
                0.0,
                0.0,
                0.213 - 0.213 * s,
                0.715 - 0.715 * s,
                0.072 + 0.928 * s,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ]
        }
        Some("hueRotate") => {
            let (sin, cos) = values
                .first()
                .copied()
                .unwrap_or(0.0)
                .to_radians()
                .sin_cos();
            [
                0.213 + cos * 0.787 - sin * 0.213,
                0.715 - cos * 0.715 - sin * 0.715,
                0.072 - cos * 0.072 + sin * 0.928,
                0.0,
                0.0,
                0.213 - cos * 0.213 + sin * 0.143,
                0.715 + cos * 0.285 + sin * 0.140,
                0.072 - cos * 0.072 - sin * 0.283,
                0.0,
                0.0,
                0.213 - cos * 0.213 - sin * 0.787,
                0.715 - cos * 0.715 + sin * 0.715,
                0.072 + cos * 0.928 + sin * 0.072,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ]
        }
        Some("luminanceToAlpha") => {
            let mut matrix = [0.0; 20];
            matrix[15] = 0.2125;
            matrix[16] = 0.7154;
            matrix[17] = 0.0721;
            matrix
        }
        _ => match <[f32; 20]>::try_from(values.as_slice()) {
            Ok(matrix) => matrix,
            Err(_) => IDENTITY,
        },
    }
}
