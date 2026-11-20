use rikka_slang::TextureKind;
use serde::{Deserialize, Serialize};

use crate::ops::{Category, Op};
use crate::types::{Constant, PortType, ValueType};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Op(Op),
    Constant(Constant),
    Parameter(String),
    Input(Input),
    Swizzle(String),
    Texture(TextureKind),
    Fbm(Noise),
    Blend(BlendMode),
    ColorRamp(Ramp),
    Curve(Curve),
    Lighting,
    Emission,
    Transparent,
    MixShader,
    AddShader,
    ShaderToRgb,
    Layer(Layer),
    Sample,
    Extern(String),
    Call(String),
    FunctionInput(String),
    FunctionOutput(String),
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionSpace {
    World,
    Object,
    View,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectionSpace {
    World,
    View,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewSpace {
    World,
    View,
    Tangent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Input {
    Position(PositionSpace),
    Normal(DirectionSpace),
    Tangent(DirectionSpace),
    ViewDirection(ViewSpace),
    Uv,
    VertexColor,
    Time,
    ScreenPosition,
    ViewDepth,
    VertexIndex,
    FrontFacing,
    SmoothedNormal,
    LightDirection,
    LightColor,
    LightAttenuation,
    Shadow,
    Ambient,
    AmbientOcclusion,
    Size,
}

impl Input {
    pub const ALL: [Self; 25] = [
        Self::Position(PositionSpace::World),
        Self::Position(PositionSpace::Object),
        Self::Position(PositionSpace::View),
        Self::Normal(DirectionSpace::World),
        Self::Normal(DirectionSpace::View),
        Self::Tangent(DirectionSpace::World),
        Self::Tangent(DirectionSpace::View),
        Self::ViewDirection(ViewSpace::World),
        Self::ViewDirection(ViewSpace::View),
        Self::ViewDirection(ViewSpace::Tangent),
        Self::Uv,
        Self::VertexColor,
        Self::Time,
        Self::ScreenPosition,
        Self::ViewDepth,
        Self::VertexIndex,
        Self::FrontFacing,
        Self::SmoothedNormal,
        Self::LightDirection,
        Self::LightColor,
        Self::LightAttenuation,
        Self::Shadow,
        Self::Ambient,
        Self::AmbientOcclusion,
        Self::Size,
    ];

    pub fn ty(self) -> ValueType {
        match self {
            Self::Position(_)
            | Self::Normal(_)
            | Self::Tangent(_)
            | Self::ViewDirection(_)
            | Self::SmoothedNormal
            | Self::LightDirection
            | Self::LightColor
            | Self::Ambient => ValueType::Float3,
            Self::Uv | Self::ScreenPosition | Self::Size => ValueType::Float2,
            Self::VertexColor => ValueType::Float4,
            Self::Time
            | Self::ViewDepth
            | Self::VertexIndex
            | Self::FrontFacing
            | Self::LightAttenuation
            | Self::Shadow
            | Self::AmbientOcclusion => ValueType::Float,
        }
    }

    pub fn title(self) -> String {
        let direction = |space: DirectionSpace| match space {
            DirectionSpace::World => "world",
            DirectionSpace::View => "view",
        };
        match self {
            Self::Position(space) => format!(
                "position ({})",
                match space {
                    PositionSpace::World => "world",
                    PositionSpace::Object => "object",
                    PositionSpace::View => "view",
                }
            ),
            Self::Normal(space) => format!("normal ({})", direction(space)),
            Self::Tangent(space) => format!("tangent ({})", direction(space)),
            Self::ViewDirection(space) => format!(
                "view direction ({})",
                match space {
                    ViewSpace::World => "world",
                    ViewSpace::View => "view",
                    ViewSpace::Tangent => "tangent",
                }
            ),
            Self::Uv => "uv".to_owned(),
            Self::VertexColor => "vertex colour".to_owned(),
            Self::Time => "time".to_owned(),
            Self::ScreenPosition => "screen position".to_owned(),
            Self::ViewDepth => "view depth".to_owned(),
            Self::VertexIndex => "vertex index".to_owned(),
            Self::FrontFacing => "front facing".to_owned(),
            Self::SmoothedNormal => "smoothed normal".to_owned(),
            Self::LightDirection => "light direction".to_owned(),
            Self::LightColor => "light colour".to_owned(),
            Self::LightAttenuation => "light attenuation".to_owned(),
            Self::Shadow => "shadow".to_owned(),
            Self::Ambient => "ambient".to_owned(),
            Self::AmbientOcclusion => "ambient occlusion".to_owned(),
            Self::Size => "size".to_owned(),
        }
    }

    pub fn doc(self) -> &'static str {
        match self {
            Self::Position(PositionSpace::World) => "The shaded point in world space.",
            Self::Position(PositionSpace::Object) => {
                "The vertex in the mesh's own space, before its transform (displacement only)."
            }
            Self::Position(PositionSpace::View) => "The shaded point relative to the camera.",
            Self::Normal(_) => "The interpolated surface normal, facing the viewer.",
            Self::Tangent(_) => "The surface tangent along the uv's u direction.",
            Self::ViewDirection(_) => "The unit direction from the shaded point to the camera.",
            Self::Uv => "The texture coordinate; in composites the pixel's place on screen, 0…1.",
            Self::VertexColor => "The mesh's vertex colour.",
            Self::Time => "The frame time in seconds.",
            Self::ScreenPosition => "The pixel's position on screen, in pixels.",
            Self::ViewDepth => "The distance in front of the camera.",
            Self::VertexIndex => "The vertex's index in its mesh (displacement only).",
            Self::FrontFacing => "1 on the front of a face, 0 on its back.",
            Self::SmoothedNormal => {
                "The normal averaged over coinciding vertices, for unbroken outlines (outline only)."
            }
            Self::LightDirection => "The unit direction towards the light being added.",
            Self::LightColor => "The light's colour times its intensity.",
            Self::LightAttenuation => "The falloff of the light with distance and cone.",
            Self::Shadow => "How much of the light reaches the point, 0 in full shadow.",
            Self::Ambient => "The frame's flat ambient colour.",
            Self::AmbientOcclusion => "The ambient occlusion, 0 where occluded.",
            Self::Size => "The image size in pixels.",
        }
    }

    pub fn category(self) -> Category {
        match self {
            Self::LightDirection
            | Self::LightColor
            | Self::LightAttenuation
            | Self::Shadow
            | Self::Ambient
            | Self::AmbientOcclusion => Category::Lighting,
            _ => Category::Input,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Image,
    Depth,
    Normals,
    Velocity,
    Layers,
    Emission,
    Occlusion,
    Previous,
}

impl Layer {
    pub const ALL: [Self; 8] = [
        Self::Image,
        Self::Depth,
        Self::Normals,
        Self::Velocity,
        Self::Layers,
        Self::Emission,
        Self::Occlusion,
        Self::Previous,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Depth => "depth",
            Self::Normals => "normals",
            Self::Velocity => "velocity",
            Self::Layers => "layers",
            Self::Emission => "emission",
            Self::Occlusion => "occlusion",
            Self::Previous => "previous",
        }
    }

    pub fn doc(self) -> &'static str {
        match self {
            Self::Image => "The rendered scene, in scene-referred linear light.",
            Self::Depth => "The scene depth at each pixel.",
            Self::Normals => "The scene's world normal at each pixel.",
            Self::Velocity => "The scene's screen motion since the last frame.",
            Self::Layers => "The object layer mask at each pixel.",
            Self::Emission => "Only the light surfaces emit.",
            Self::Occlusion => "The ambient occlusion, white where open.",
            Self::Previous => "What this composite showed on the previous frame.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Noise {
    Value,
    Gradient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    SoftLight,
    Add,
    Subtract,
    Difference,
    Darken,
    Lighten,
    Dodge,
    Burn,
    Divide,
    LinearLight,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Value,
}

impl BlendMode {
    pub const ALL: [Self; 19] = [
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::SoftLight,
        Self::Add,
        Self::Subtract,
        Self::Difference,
        Self::Darken,
        Self::Lighten,
        Self::Dodge,
        Self::Burn,
        Self::Divide,
        Self::LinearLight,
        Self::Exclusion,
        Self::Hue,
        Self::Saturation,
        Self::Color,
        Self::Value,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interpolation {
    #[default]
    Linear,
    Smooth,
    Constant,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    pub position: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ramp {
    pub stops: Vec<Stop>,
    #[serde(default)]
    pub interpolation: Interpolation,
}

impl Default for Ramp {
    fn default() -> Self {
        Self {
            stops: vec![
                Stop {
                    position: 0.0,
                    color: [0.0, 0.0, 0.0, 1.0],
                },
                Stop {
                    position: 1.0,
                    color: [1.0, 1.0, 1.0, 1.0],
                },
            ],
            interpolation: Interpolation::Linear,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    pub points: Vec<[f32; 2]>,
    #[serde(default)]
    pub interpolation: Interpolation,
}

impl Default for Curve {
    fn default() -> Self {
        Self {
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            interpolation: Interpolation::Linear,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortDefault {
    Value(Constant),
    Input(Input),
    Required,
    Empty,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Hint {
    #[default]
    None,
    Color,
    Angle,
    Toggle,
    Integer,
    Range(f32, f32),
    Soft(f32, f32),
    Options(&'static [&'static str]),
    Texture(TextureKind),
    File(&'static [&'static str]),
    Text,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Port {
    pub name: String,
    pub ty: PortType,
    pub default: PortDefault,
    pub hint: Hint,
    pub many: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Ports {
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
}

impl Ports {
    pub fn input(&self, name: &str) -> Option<&Port> {
        self.inputs.iter().find(|port| port.name == name)
    }

    pub fn output(&self, name: &str) -> Option<&Port> {
        self.outputs.iter().find(|port| port.name == name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortSpec {
    pub name: &'static str,
    pub ty: PortType,
    pub default: PortDefault,
    pub hint: Hint,
    pub many: bool,
}

impl PortSpec {
    pub const fn new(name: &'static str, ty: PortType, default: PortDefault) -> Self {
        Self {
            name,
            ty,
            default,
            hint: Hint::None,
            many: false,
        }
    }

    pub const fn number(name: &'static str, value: f32) -> Self {
        Self::new(
            name,
            PortType::Value(ValueType::Float),
            PortDefault::Value(Constant::scalar(value)),
        )
    }

    pub const fn vector(name: &'static str, x: f32, y: f32, z: f32) -> Self {
        Self::new(
            name,
            PortType::Value(ValueType::Float3),
            PortDefault::Value(Constant::vec3(x, y, z)),
        )
    }

    pub const fn color(name: &'static str, r: f32, g: f32, b: f32) -> Self {
        Self::vector(name, r, g, b).hint(Hint::Color)
    }

    pub const fn toggle(name: &'static str, on: bool) -> Self {
        Self::number(name, if on { 1.0 } else { 0.0 }).hint(Hint::Toggle)
    }

    pub const fn options(
        name: &'static str,
        options: &'static [&'static str],
        chosen: usize,
    ) -> Self {
        Self::number(name, chosen as f32).hint(Hint::Options(options))
    }

    pub const fn value(name: &'static str, ty: ValueType, default: Constant) -> Self {
        Self::new(name, PortType::Value(ty), PortDefault::Value(default))
    }

    pub const fn dynamic(name: &'static str, value: f32) -> Self {
        Self::new(
            name,
            PortType::Dynamic,
            PortDefault::Value(Constant::scalar(value)),
        )
    }

    pub const fn context(name: &'static str, ty: ValueType, input: Input) -> Self {
        Self::new(name, PortType::Value(ty), PortDefault::Input(input))
    }

    pub const fn required(name: &'static str, ty: ValueType) -> Self {
        Self::new(name, PortType::Value(ty), PortDefault::Required)
    }

    pub const fn empty(name: &'static str, ty: ValueType) -> Self {
        Self::new(name, PortType::Value(ty), PortDefault::Empty)
    }

    pub const fn output(name: &'static str, ty: ValueType) -> Self {
        Self::new(name, PortType::Value(ty), PortDefault::Required)
    }

    pub const fn hint(mut self, hint: Hint) -> Self {
        self.hint = hint;
        self
    }

    pub const fn range(self, low: f32, high: f32) -> Self {
        self.hint(Hint::Range(low, high))
    }

    pub const fn soft(self, low: f32, high: f32) -> Self {
        self.hint(Hint::Soft(low, high))
    }

    pub const fn many(mut self) -> Self {
        self.many = true;
        self
    }

    pub fn port(&self) -> Port {
        Port {
            name: self.name.to_owned(),
            ty: self.ty,
            default: self.default,
            hint: self.hint,
            many: self.many,
        }
    }
}

pub(crate) const fn dynamic(name: &'static str, value: f32) -> PortSpec {
    PortSpec::dynamic(name, value)
}

pub(crate) const fn value(name: &'static str, ty: ValueType, default: Constant) -> PortSpec {
    PortSpec::value(name, ty, default)
}

pub(crate) const fn context(name: &'static str, ty: ValueType, input: Input) -> PortSpec {
    PortSpec::context(name, ty, input)
}

pub(crate) const fn out(name: &'static str, ty: PortType) -> PortSpec {
    PortSpec::new(name, ty, PortDefault::Required)
}
