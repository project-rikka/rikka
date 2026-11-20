mod color;
mod range;
mod texture;
mod vector;

use serde::{Deserialize, Serialize};

use crate::node::{
    DirectionSpace, Input, PortDefault, PortSpec, PositionSpace, ViewSpace, context, dynamic, out,
    value,
};
use crate::types::{Constant, PortType, ValueType};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Minimum,
    Maximum,
    Modulo,
    Clamp,
    Saturate,
    Negate,
    Absolute,
    Floor,
    Ceil,
    Fraction,
    Round,
    Sign,
    SquareRoot,
    Exponential,
    Logarithm,
    OneMinus,
    Reciprocal,
    Step,
    Lerp,
    Smoothstep,
    InverseLerp,
    Remap,
    Sine,
    Cosine,
    Tangent,
    ArcSine,
    ArcCosine,
    ArcTangent,
    ArcTangent2,
    Radians,
    Degrees,
    Dot,
    Cross,
    Length,
    Distance,
    Normalize,
    Reflect,
    Refract,
    Combine,
    Tiling,
    Rotate,
    Polar,
    Parallax,
    ValueNoise,
    GradientNoise,
    Voronoi,
    Fresnel,
    NormalMap,
    RgbToHsv,
    HsvToRgb,
    Luminance,
    Saturation,
    SkyIrradiance,
    SkyReflection,
    IndirectDiffuse,
    MapRange,
    Compare,
    Switch,
    ComponentMin,
    ComponentMax,
    WhiteNoise,
    Checker,
    Brick,
    Wave,
    Gradient,
    Mapping,
    VectorRotate,
    HueSaturation,
    BrightnessContrast,
    Gamma,
    Invert,
    Blackbody,
    Wavelength,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Math,
    Interpolation,
    Trigonometry,
    Vector,
    Uv,
    Noise,
    Surface,
    Color,
    Ramp,
    Texture,
    Lighting,
    Shader,
    Toon,
    Layer,
    Filter,
    Lens,
    Stylize,
    Environment,
    Light,
    Render,
    Stage,
    Input,
    Parameter,
    Constant,
    Function,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Needs {
    Nothing,
    Pixel,
    Indirect,
}

pub(crate) struct OpInfo {
    pub title: &'static str,
    pub category: Category,
    pub doc: &'static str,
    pub inputs: Vec<PortSpec>,
    pub outputs: Vec<PortSpec>,
    pub needs: Needs,
}

impl OpInfo {
    pub(crate) fn new(
        title: &'static str,
        category: Category,
        doc: &'static str,
        inputs: &[PortSpec],
        outputs: &[PortSpec],
    ) -> Self {
        Self {
            title,
            category,
            doc,
            inputs: inputs.to_vec(),
            outputs: outputs.to_vec(),
            needs: Needs::Nothing,
        }
    }
}

const RESULT: &[PortSpec] = &[out("value", PortType::Dynamic)];
const SCALAR: &[PortSpec] = &[out("value", PortType::Value(ValueType::Float))];
const PLANE: &[PortSpec] = &[out("value", PortType::Value(ValueType::Float2))];
const SPATIAL: &[PortSpec] = &[out("value", PortType::Value(ValueType::Float3))];
const UNARY: &[PortSpec] = &[dynamic("value", 0.0)];
const PAIR: &[PortSpec] = &[dynamic("a", 0.0), dynamic("b", 0.0)];
const FACTORS: &[PortSpec] = &[dynamic("a", 1.0), dynamic("b", 1.0)];
const UV: PortSpec = context("uv", ValueType::Float2, Input::Uv);
const CENTER: PortSpec = value("center", ValueType::Float2, Constant::vec2(0.5, 0.5));
pub(crate) const POSITION: PortSpec = PortSpec::new(
    "position",
    PortType::Dynamic,
    PortDefault::Input(Input::Position(PositionSpace::World)),
);
const SCALE: PortSpec = value("scale", ValueType::Float, Constant::scalar(1.0));
const NORMAL: PortSpec = context(
    "normal",
    ValueType::Float3,
    Input::Normal(DirectionSpace::World),
);
const COLOR: PortSpec = value("color", ValueType::Float3, Constant::vec3(1.0, 1.0, 1.0));

impl Op {
    pub const ALL: [Self; 78] = [
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Divide,
        Self::Power,
        Self::Minimum,
        Self::Maximum,
        Self::Modulo,
        Self::Clamp,
        Self::Saturate,
        Self::Negate,
        Self::Absolute,
        Self::Floor,
        Self::Ceil,
        Self::Fraction,
        Self::Round,
        Self::Sign,
        Self::SquareRoot,
        Self::Exponential,
        Self::Logarithm,
        Self::OneMinus,
        Self::Reciprocal,
        Self::Step,
        Self::Lerp,
        Self::Smoothstep,
        Self::InverseLerp,
        Self::Remap,
        Self::Sine,
        Self::Cosine,
        Self::Tangent,
        Self::ArcSine,
        Self::ArcCosine,
        Self::ArcTangent,
        Self::ArcTangent2,
        Self::Radians,
        Self::Degrees,
        Self::Dot,
        Self::Cross,
        Self::Length,
        Self::Distance,
        Self::Normalize,
        Self::Reflect,
        Self::Refract,
        Self::Combine,
        Self::Tiling,
        Self::Rotate,
        Self::Polar,
        Self::Parallax,
        Self::ValueNoise,
        Self::GradientNoise,
        Self::Voronoi,
        Self::Fresnel,
        Self::NormalMap,
        Self::RgbToHsv,
        Self::HsvToRgb,
        Self::Luminance,
        Self::Saturation,
        Self::SkyIrradiance,
        Self::SkyReflection,
        Self::IndirectDiffuse,
        Self::MapRange,
        Self::Compare,
        Self::Switch,
        Self::ComponentMin,
        Self::ComponentMax,
        Self::WhiteNoise,
        Self::Checker,
        Self::Brick,
        Self::Wave,
        Self::Gradient,
        Self::Mapping,
        Self::VectorRotate,
        Self::HueSaturation,
        Self::BrightnessContrast,
        Self::Gamma,
        Self::Invert,
        Self::Blackbody,
        Self::Wavelength,
    ];

    pub(crate) fn info(self) -> OpInfo {
        let info = |title: &'static str,
                    category: Category,
                    doc: &'static str,
                    inputs: &[PortSpec],
                    outputs: &[PortSpec]| OpInfo {
            title,
            category,
            doc,
            inputs: inputs.to_vec(),
            outputs: outputs.to_vec(),
            needs: Needs::Nothing,
        };
        let math =
            |title, doc, inputs: &[PortSpec]| info(title, Category::Math, doc, inputs, RESULT);
        let trig = |title, doc| info(title, Category::Trigonometry, doc, UNARY, RESULT);
        match self {
            Self::Add => math("Add", "a + b, per component.", PAIR),
            Self::Subtract => math("Subtract", "a - b, per component.", PAIR),
            Self::Multiply => math("Multiply", "a × b, per component.", FACTORS),
            Self::Divide => math("Divide", "a ÷ b, per component.", FACTORS),
            Self::Power => math(
                "Power",
                "base raised to exponent.",
                &[dynamic("base", 1.0), dynamic("exponent", 1.0)],
            ),
            Self::Minimum => math("Minimum", "The smaller of a and b.", PAIR),
            Self::Maximum => math("Maximum", "The larger of a and b.", PAIR),
            Self::Modulo => math(
                "Modulo",
                "The remainder of a ÷ b, with the sign of a.",
                FACTORS,
            ),
            Self::Clamp => math(
                "Clamp",
                "value limited to low…high.",
                &[
                    dynamic("value", 0.0),
                    dynamic("low", 0.0),
                    dynamic("high", 1.0),
                ],
            ),
            Self::Saturate => math("Saturate", "value limited to 0…1.", UNARY),
            Self::Negate => math("Negate", "-value.", UNARY),
            Self::Absolute => math("Absolute", "|value|.", UNARY),
            Self::Floor => math("Floor", "The largest whole number ≤ value.", UNARY),
            Self::Ceil => math("Ceil", "The smallest whole number ≥ value.", UNARY),
            Self::Fraction => math("Fraction", "value - floor(value).", UNARY),
            Self::Round => math("Round", "The nearest whole number.", UNARY),
            Self::Sign => math("Sign", "-1, 0 or 1 by the sign of value.", UNARY),
            Self::SquareRoot => math("Square root", "√value.", UNARY),
            Self::Exponential => math("Exponential", "e raised to value.", UNARY),
            Self::Logarithm => math("Logarithm", "The natural logarithm.", UNARY),
            Self::OneMinus => math("One minus", "1 - value.", UNARY),
            Self::Reciprocal => math("Reciprocal", "1 ÷ value.", &[dynamic("value", 1.0)]),
            Self::Step => math(
                "Step",
                "1 where value ≥ edge, else 0.",
                &[dynamic("edge", 0.5), dynamic("value", 0.0)],
            ),
            Self::Lerp => info(
                "Lerp",
                Category::Interpolation,
                "Blends a towards b by t.",
                &[dynamic("a", 0.0), dynamic("b", 1.0), dynamic("t", 0.5)],
                RESULT,
            ),
            Self::Smoothstep => info(
                "Smoothstep",
                Category::Interpolation,
                "A smooth 0…1 ramp of value between low and high.",
                &[
                    dynamic("low", 0.0),
                    dynamic("high", 1.0),
                    dynamic("value", 0.5),
                ],
                RESULT,
            ),
            Self::InverseLerp => info(
                "Inverse lerp",
                Category::Interpolation,
                "Where value lies between a and b, as 0…1.",
                &[dynamic("a", 0.0), dynamic("b", 1.0), dynamic("value", 0.5)],
                RESULT,
            ),
            Self::Remap => info(
                "Remap",
                Category::Interpolation,
                "Maps value from one range onto another.",
                &[
                    dynamic("value", 0.5),
                    dynamic("from_low", 0.0),
                    dynamic("from_high", 1.0),
                    dynamic("to_low", 0.0),
                    dynamic("to_high", 1.0),
                ],
                RESULT,
            ),
            Self::Sine => trig("Sine", "sin(value), in radians."),
            Self::Cosine => trig("Cosine", "cos(value), in radians."),
            Self::Tangent => trig("Tangent", "tan(value), in radians."),
            Self::ArcSine => trig("Arcsine", "asin(value), in radians."),
            Self::ArcCosine => trig("Arccosine", "acos(value), in radians."),
            Self::ArcTangent => trig("Arctangent", "atan(value), in radians."),
            Self::ArcTangent2 => info(
                "Arctangent 2",
                Category::Trigonometry,
                "The angle of (x, y), in radians.",
                &[dynamic("y", 0.0), dynamic("x", 1.0)],
                RESULT,
            ),
            Self::Radians => trig("Radians", "Degrees to radians."),
            Self::Degrees => trig("Degrees", "Radians to degrees."),
            Self::Dot => info(
                "Dot",
                Category::Vector,
                "The dot product of a and b.",
                PAIR,
                SCALAR,
            ),
            Self::Cross => info(
                "Cross",
                Category::Vector,
                "The cross product of a and b.",
                &[
                    value("a", ValueType::Float3, Constant::vec3(1.0, 0.0, 0.0)),
                    value("b", ValueType::Float3, Constant::vec3(0.0, 1.0, 0.0)),
                ],
                SPATIAL,
            ),
            Self::Length => info(
                "Length",
                Category::Vector,
                "The length of value.",
                UNARY,
                SCALAR,
            ),
            Self::Distance => info(
                "Distance",
                Category::Vector,
                "The distance between a and b.",
                PAIR,
                SCALAR,
            ),
            Self::Normalize => info(
                "Normalize",
                Category::Vector,
                "value scaled to length 1.",
                &[dynamic("value", 1.0)],
                RESULT,
            ),
            Self::Reflect => info(
                "Reflect",
                Category::Vector,
                "incident mirrored about normal.",
                &[dynamic("incident", 0.0), dynamic("normal", 1.0)],
                RESULT,
            ),
            Self::Refract => info(
                "Refract",
                Category::Vector,
                "incident bent through normal by the index ratio eta.",
                &[
                    dynamic("incident", 0.0),
                    dynamic("normal", 1.0),
                    value("eta", ValueType::Float, Constant::scalar(1.0)),
                ],
                RESULT,
            ),
            Self::Combine => info(
                "Combine",
                Category::Vector,
                "Builds vectors from components.",
                &[
                    value("x", ValueType::Float, Constant::scalar(0.0)),
                    value("y", ValueType::Float, Constant::scalar(0.0)),
                    value("z", ValueType::Float, Constant::scalar(0.0)),
                    value("w", ValueType::Float, Constant::scalar(0.0)),
                ],
                &[
                    out("xy", PortType::Value(ValueType::Float2)),
                    out("xyz", PortType::Value(ValueType::Float3)),
                    out("xyzw", PortType::Value(ValueType::Float4)),
                ],
            ),
            Self::Tiling => info(
                "Tiling and offset",
                Category::Uv,
                "uv × tiling + offset.",
                &[
                    UV,
                    value("tiling", ValueType::Float2, Constant::vec2(1.0, 1.0)),
                    value("offset", ValueType::Float2, Constant::vec2(0.0, 0.0)),
                ],
                PLANE,
            ),
            Self::Rotate => info(
                "Rotate",
                Category::Uv,
                "uv turned about center by angle radians.",
                &[
                    UV,
                    CENTER,
                    value("angle", ValueType::Float, Constant::scalar(0.0)),
                ],
                PLANE,
            ),
            Self::Polar => info(
                "Polar",
                Category::Uv,
                "uv as (radius, angle) about center, the angle as 0…1.",
                &[UV, CENTER],
                PLANE,
            ),
            Self::Parallax => OpInfo {
                needs: Needs::Pixel,
                ..info(
                    "Parallax",
                    Category::Uv,
                    "uv shifted along the tangent-space view by height × scale.",
                    &[
                        UV,
                        value("height", ValueType::Float, Constant::scalar(0.5)),
                        value("scale", ValueType::Float, Constant::scalar(0.05)),
                        context(
                            "view",
                            ValueType::Float3,
                            Input::ViewDirection(ViewSpace::Tangent),
                        ),
                    ],
                    PLANE,
                )
            },
            Self::ValueNoise => info(
                "Value noise",
                Category::Noise,
                "Smoothly interpolated random values in 0…1.",
                &[POSITION, SCALE],
                SCALAR,
            ),
            Self::GradientNoise => info(
                "Gradient noise",
                Category::Noise,
                "Perlin-style gradient noise in 0…1.",
                &[POSITION, SCALE],
                SCALAR,
            ),
            Self::Voronoi => info(
                "Voronoi",
                Category::Noise,
                "The distance to the nearest random cell point, and that cell's random value.",
                &[
                    POSITION,
                    SCALE,
                    value("randomness", ValueType::Float, Constant::scalar(1.0)),
                ],
                &[
                    out("distance", PortType::Value(ValueType::Float)),
                    out("cell", PortType::Value(ValueType::Float)),
                ],
            ),
            Self::Fresnel => info(
                "Fresnel",
                Category::Surface,
                "(1 - n·v) raised to power: bright where the surface turns away.",
                &[
                    NORMAL,
                    context(
                        "view",
                        ValueType::Float3,
                        Input::ViewDirection(ViewSpace::World),
                    ),
                    value("power", ValueType::Float, Constant::scalar(5.0)),
                ],
                SCALAR,
            ),
            Self::NormalMap => OpInfo {
                needs: Needs::Pixel,
                ..info(
                    "Normal map",
                    Category::Surface,
                    "A tangent-space normal turned into a world normal.",
                    &[
                        value("normal", ValueType::Float3, Constant::vec3(0.0, 0.0, 1.0)),
                        value("strength", ValueType::Float, Constant::scalar(1.0)),
                    ],
                    SPATIAL,
                )
            },
            Self::RgbToHsv => info(
                "RGB to HSV",
                Category::Color,
                "Hue, saturation and value, each 0…1.",
                &[COLOR],
                SPATIAL,
            ),
            Self::HsvToRgb => info(
                "HSV to RGB",
                Category::Color,
                "Red, green and blue from hue, saturation and value.",
                &[value(
                    "color",
                    ValueType::Float3,
                    Constant::vec3(0.0, 0.0, 1.0),
                )],
                SPATIAL,
            ),
            Self::Luminance => info(
                "Luminance",
                Category::Color,
                "The Rec. 709 brightness of color.",
                &[COLOR],
                SCALAR,
            ),
            Self::Saturation => info(
                "Saturation",
                Category::Color,
                "color pushed towards grey (0) or beyond (above 1).",
                &[
                    COLOR,
                    value("amount", ValueType::Float, Constant::scalar(1.0)),
                ],
                SPATIAL,
            ),
            Self::SkyIrradiance => OpInfo {
                needs: Needs::Indirect,
                ..info(
                    "Sky irradiance",
                    Category::Lighting,
                    "Diffuse light from the sky around normal.",
                    &[NORMAL],
                    SPATIAL,
                )
            },
            Self::SkyReflection => OpInfo {
                needs: Needs::Indirect,
                ..info(
                    "Sky reflection",
                    Category::Lighting,
                    "Light reflected from the sky along direction, blurred by roughness.",
                    &[
                        context(
                            "direction",
                            ValueType::Float3,
                            Input::Normal(DirectionSpace::World),
                        ),
                        value("roughness", ValueType::Float, Constant::scalar(0.5)),
                    ],
                    SPATIAL,
                )
            },
            Self::IndirectDiffuse => OpInfo {
                needs: Needs::Indirect,
                ..info(
                    "Indirect diffuse",
                    Category::Lighting,
                    "Diffuse light bounced in from the scene around normal: traced light when the frame traces it, else the occluded sky.",
                    &[NORMAL],
                    SPATIAL,
                )
            },
            Self::MapRange
            | Self::Compare
            | Self::Switch
            | Self::ComponentMin
            | Self::ComponentMax => range::info(self),
            Self::WhiteNoise | Self::Checker | Self::Brick | Self::Wave | Self::Gradient => {
                texture::info(self)
            }
            Self::Mapping | Self::VectorRotate => vector::info(self),
            Self::HueSaturation
            | Self::BrightnessContrast
            | Self::Gamma
            | Self::Invert
            | Self::Blackbody
            | Self::Wavelength => color::info(self),
        }
    }

    pub(crate) fn expression(self, args: &[String], ty: ValueType) -> String {
        let a = |index: usize| args[index].as_str();
        let call = |name: &str| format!("{name}({})", args.join(", "));
        match self {
            Self::Add => format!("({} + {})", a(0), a(1)),
            Self::Subtract => format!("({} - {})", a(0), a(1)),
            Self::Multiply => format!("({} * {})", a(0), a(1)),
            Self::Divide => format!("({} / {})", a(0), a(1)),
            Self::Power => call("pow"),
            Self::Minimum => call("min"),
            Self::Maximum => call("max"),
            Self::Modulo => call("fmod"),
            Self::Clamp => call("clamp"),
            Self::Saturate => call("saturate"),
            Self::Negate => format!("(-{})", a(0)),
            Self::Absolute => call("abs"),
            Self::Floor => call("floor"),
            Self::Ceil => call("ceil"),
            Self::Fraction => call("frac"),
            Self::Round => call("round"),
            Self::Sign => format!("{}(sign({}))", ty.slang(), a(0)),
            Self::SquareRoot => call("sqrt"),
            Self::Exponential => call("exp"),
            Self::Logarithm => call("log"),
            Self::OneMinus => format!("(1.0 - {})", a(0)),
            Self::Reciprocal => format!("(1.0 / {})", a(0)),
            Self::Step => call("step"),
            Self::Lerp => call("lerp"),
            Self::Smoothstep => call("smoothstep"),
            Self::InverseLerp => format!("(({} - {}) / ({} - {}))", a(2), a(0), a(1), a(0)),
            Self::Remap => format!(
                "lerp({}, {}, ({} - {}) / ({} - {}))",
                a(3),
                a(4),
                a(0),
                a(1),
                a(2),
                a(1)
            ),
            Self::Sine => call("sin"),
            Self::Cosine => call("cos"),
            Self::Tangent => call("tan"),
            Self::ArcSine => call("asin"),
            Self::ArcCosine => call("acos"),
            Self::ArcTangent => call("atan"),
            Self::ArcTangent2 => call("atan2"),
            Self::Radians => call("radians"),
            Self::Degrees => call("degrees"),
            Self::Dot => call("dot"),
            Self::Cross => call("cross"),
            Self::Length => call("length"),
            Self::Distance => call("distance"),
            Self::Normalize => call("normalize"),
            Self::Reflect => call("reflect"),
            Self::Refract => call("refract"),
            Self::Combine => call("float4"),
            Self::Tiling => format!("({} * {} + {})", a(0), a(1), a(2)),
            Self::Rotate => call("rikka_graph_rotate"),
            Self::Polar => call("rikka_graph_polar"),
            Self::Parallax => call("rikka_graph_parallax"),
            Self::ValueNoise => format!("rikka_graph_value_noise({} * {})", a(0), a(1)),
            Self::GradientNoise => format!("rikka_graph_gradient_noise({} * {})", a(0), a(1)),
            Self::Voronoi => format!("rikka_graph_voronoi({} * {}, {})", a(0), a(1), a(2)),
            Self::Fresnel => format!("pow(1.0 - saturate(dot({}, {})), {})", a(0), a(1), a(2)),
            Self::NormalMap => unreachable!("normal maps read the pixel"),
            Self::RgbToHsv => call("rikka_graph_rgb_to_hsv"),
            Self::HsvToRgb => call("rikka_graph_hsv_to_rgb"),
            Self::Luminance => call("rikka_luma"),
            Self::Saturation => call("rikka_graph_saturation"),
            Self::SkyIrradiance => format!("indirect.irradiance({})", a(0)),
            Self::SkyReflection => format!("indirect.specular({}, {})", a(0), a(1)),
            Self::IndirectDiffuse => format!("indirect.diffuse({})", a(0)),
            Self::MapRange
            | Self::Compare
            | Self::Switch
            | Self::ComponentMin
            | Self::ComponentMax => range::expression(self, args, ty),
            Self::WhiteNoise | Self::Checker | Self::Brick | Self::Wave | Self::Gradient => {
                texture::expression(self, args)
            }
            Self::Mapping | Self::VectorRotate => vector::expression(self, args),
            Self::HueSaturation
            | Self::BrightnessContrast
            | Self::Gamma
            | Self::Invert
            | Self::Blackbody
            | Self::Wavelength => color::expression(self, args),
        }
    }

    pub(crate) fn reads_point(self) -> bool {
        matches!(
            self,
            Self::ValueNoise
                | Self::GradientNoise
                | Self::Voronoi
                | Self::WhiteNoise
                | Self::Checker
                | Self::Brick
                | Self::Wave
                | Self::Gradient
        )
    }

    pub(crate) fn packed(self) -> Option<(ValueType, &'static [(&'static str, &'static str)])> {
        match self {
            Self::Combine => Some((
                ValueType::Float4,
                &[("xy", ".xy"), ("xyz", ".xyz"), ("xyzw", "")],
            )),
            Self::Voronoi => Some((ValueType::Float2, &[("distance", ".x"), ("cell", ".y")])),
            _ => texture::packed(self),
        }
    }

    pub(crate) fn fold(self, args: &[Constant], width: usize) -> Option<Vec<Constant>> {
        self.compute(args, width)
            .filter(|folded| folded.iter().all(Constant::is_finite))
    }

    pub(crate) fn compute(self, args: &[Constant], width: usize) -> Option<Vec<Constant>> {
        let each = |f: &dyn Fn(&[f32]) -> f32| {
            let mut values = [0.0; 4];
            for (index, value) in values.iter_mut().enumerate().take(width) {
                let at: Vec<f32> = args.iter().map(|arg| arg.component(index)).collect();
                *value = f(&at);
            }
            vec![Constant::from_slice(&values[..width]).expect("a folded width")]
        };
        let scalar = |value: f32| vec![Constant::scalar(value)];
        let dot = |a: &Constant, b: &Constant| -> f32 {
            (0..width).map(|i| a.component(i) * b.component(i)).sum()
        };
        let folded = match self {
            Self::Add => each(&|v| v[0] + v[1]),
            Self::Subtract => each(&|v| v[0] - v[1]),
            Self::Multiply => each(&|v| v[0] * v[1]),
            Self::Divide => each(&|v| v[0] / v[1]),
            Self::Power => each(&|v| v[0].powf(v[1])),
            Self::Minimum => each(&|v| v[0].min(v[1])),
            Self::Maximum => each(&|v| v[0].max(v[1])),
            Self::Modulo => each(&|v| v[0] % v[1]),
            Self::Clamp => each(&|v| v[0].max(v[1]).min(v[2])),
            Self::Saturate => each(&|v| v[0].clamp(0.0, 1.0)),
            Self::Negate => each(&|v| -v[0]),
            Self::Absolute => each(&|v| v[0].abs()),
            Self::Floor => each(&|v| v[0].floor()),
            Self::Ceil => each(&|v| v[0].ceil()),
            Self::Fraction => each(&|v| v[0] - v[0].floor()),
            Self::Round => each(&|v| v[0].round_ties_even()),
            Self::Sign => each(&|v| {
                if v[0] > 0.0 {
                    1.0
                } else if v[0] < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }),
            Self::SquareRoot => each(&|v| v[0].sqrt()),
            Self::Exponential => each(&|v| v[0].exp()),
            Self::Logarithm => each(&|v| v[0].ln()),
            Self::OneMinus => each(&|v| 1.0 - v[0]),
            Self::Reciprocal => each(&|v| 1.0 / v[0]),
            Self::Step => each(&|v| if v[1] >= v[0] { 1.0 } else { 0.0 }),
            Self::Lerp => each(&|v| v[0] + (v[1] - v[0]) * v[2]),
            Self::Smoothstep => each(&|v| {
                let t = ((v[2] - v[0]) / (v[1] - v[0])).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            }),
            Self::InverseLerp => each(&|v| (v[2] - v[0]) / (v[1] - v[0])),
            Self::Remap => each(&|v| v[3] + (v[4] - v[3]) * ((v[0] - v[1]) / (v[2] - v[1]))),
            Self::Sine => each(&|v| v[0].sin()),
            Self::Cosine => each(&|v| v[0].cos()),
            Self::Tangent => each(&|v| v[0].tan()),
            Self::ArcSine => each(&|v| v[0].asin()),
            Self::ArcCosine => each(&|v| v[0].acos()),
            Self::ArcTangent => each(&|v| v[0].atan()),
            Self::ArcTangent2 => each(&|v| v[0].atan2(v[1])),
            Self::Radians => each(&|v| v[0].to_radians()),
            Self::Degrees => each(&|v| v[0].to_degrees()),
            Self::Dot => scalar(dot(&args[0], &args[1])),
            Self::Length => scalar(dot(&args[0], &args[0]).sqrt()),
            Self::Distance => {
                let gap: Vec<f32> = (0..width)
                    .map(|i| args[0].component(i) - args[1].component(i))
                    .collect();
                scalar(gap.iter().map(|value| value * value).sum::<f32>().sqrt())
            }
            Self::Normalize => {
                let length = dot(&args[0], &args[0]).sqrt();
                each(&|v| v[0] / length)
            }
            Self::Cross => {
                let [a, b] = [&args[0], &args[1]]
                    .map(|arg| [arg.component(0), arg.component(1), arg.component(2)]);
                vec![Constant::vec3(
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                )]
            }
            Self::Combine => {
                let [x, y, z, w] = [0, 1, 2, 3].map(|index| args[index].component(0));
                vec![
                    Constant::vec2(x, y),
                    Constant::vec3(x, y, z),
                    Constant::vec4(x, y, z, w),
                ]
            }
            Self::Tiling => {
                let [uv, tiling, offset] = [0, 1, 2].map(|index| &args[index]);
                vec![Constant::vec2(
                    uv.component(0) * tiling.component(0) + offset.component(0),
                    uv.component(1) * tiling.component(1) + offset.component(1),
                )]
            }
            Self::Luminance => scalar(
                0.2126 * args[0].component(0)
                    + 0.7152 * args[0].component(1)
                    + 0.0722 * args[0].component(2),
            ),
            Self::MapRange
            | Self::Compare
            | Self::Switch
            | Self::ComponentMin
            | Self::ComponentMax => range::compute(self, args, width),
            Self::WhiteNoise | Self::Checker | Self::Brick | Self::Wave | Self::Gradient => {
                texture::compute(self, args)
            }
            Self::Mapping | Self::VectorRotate => vector::compute(self, args),
            Self::HueSaturation
            | Self::BrightnessContrast
            | Self::Gamma
            | Self::Invert
            | Self::Blackbody
            | Self::Wavelength => color::compute(self, args),
            _ => return crate::eval::compute(self, args, width),
        };
        Some(folded)
    }
}
