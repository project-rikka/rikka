use rikka_gpu::Filtering;
use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Literal, ParameterKind, ParameterSpec, TextureBinding, TextureKind, Value as Setting,
};
use serde::{Deserialize, Serialize};

use rikka_shader_graph::{Hint, SocketValue};

use crate::asset::Asset;
use crate::error::ValueError;
use crate::name::Name;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Texture {
    pub asset: Asset,
    pub kind: TextureKind,
    pub filtering: Filtering,
}

impl Texture {
    #[must_use]
    pub fn new(asset: &str, kind: TextureKind) -> Self {
        Self {
            asset: Asset::new(asset),
            kind,
            filtering: Filtering::default(),
        }
    }

    #[must_use]
    pub fn is_color(&self) -> bool {
        self.kind == TextureKind::Colour
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LookValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    UInt(u32),
    Bool(bool),
    Enum(Name),
    Choice(u32),
    Asset(Asset),
    Texture(Option<Texture>),
}

impl LookValue {
    #[must_use]
    pub fn from_socket(value: &SocketValue, hint: Hint) -> Self {
        match (value, hint) {
            (SocketValue::Asset(path), _) => Self::Asset(Asset::new(path)),
            (SocketValue::Number(number), Hint::Toggle) => Self::Bool(number.component(0) > 0.5),
            (SocketValue::Number(number), Hint::Options(_)) => {
                Self::Choice(number.component(0).round().max(0.0) as u32)
            }
            (SocketValue::Number(number), _) => match *number.values() {
                [x] => Self::Float(x),
                [x, y] => Self::Vec2([x, y]),
                [x, y, z] => Self::Vec3([x, y, z]),
                [x, y, z, w] => Self::Vec4([x, y, z, w]),
                _ => unreachable!("a constant holds one to four values"),
            },
        }
    }

    #[must_use]
    pub fn to_socket(&self) -> Option<SocketValue> {
        Some(match self {
            Self::Float(value) => SocketValue::from(*value),
            Self::Vec2(value) => SocketValue::from(*value),
            Self::Vec3(value) => SocketValue::from(*value),
            Self::Vec4(value) => SocketValue::from(*value),
            Self::Int(value) => SocketValue::from(*value as f32),
            Self::UInt(value) | Self::Choice(value) => SocketValue::from(*value as f32),
            Self::Bool(on) => SocketValue::from(if *on { 1.0 } else { 0.0 }),
            Self::Asset(path) => SocketValue::Asset(path.to_string()),
            Self::Enum(_) | Self::Texture(_) => return None,
        })
    }

    #[must_use]
    pub fn mix(&self, to: &Self, t: f32) -> Self {
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        match (self.clone(), to.clone()) {
            (Self::Float(a), Self::Float(b)) => Self::Float(lerp(a, b)),
            (Self::Vec2(a), Self::Vec2(b)) => {
                Self::Vec2(std::array::from_fn(|at| lerp(a[at], b[at])))
            }
            (Self::Vec3(a), Self::Vec3(b)) => {
                Self::Vec3(std::array::from_fn(|at| lerp(a[at], b[at])))
            }
            (Self::Vec4(a), Self::Vec4(b)) => {
                Self::Vec4(std::array::from_fn(|at| lerp(a[at], b[at])))
            }
            (Self::Int(a), Self::Int(b)) => Self::Int(lerp(a as f32, b as f32).round() as i32),
            (Self::UInt(a), Self::UInt(b)) => Self::UInt(lerp(a as f32, b as f32).round() as u32),
            (held, _) => held,
        }
    }

    #[must_use]
    pub fn from_literal(spec: &ParameterSpec, literal: Literal) -> Self {
        match (literal, &spec.kind) {
            (Literal::Float(value), _) => Self::Float(value),
            (Literal::Vec2(value), _) => Self::Vec2(value),
            (Literal::Vec3(value), _) => Self::Vec3(value),
            (Literal::Vec4(value), _) => Self::Vec4(value),
            (Literal::Int(value), _) => Self::Int(value),
            (Literal::UInt(value), _) => Self::UInt(value),
            (Literal::Bool(value), _) => Self::Bool(value),
            (Literal::Enum(index), ParameterKind::Enum { options }) => options
                .get(index as usize)
                .map_or(Self::UInt(index), |option| Self::Enum(Name::new(option))),
            (Literal::Enum(index), _) => Self::UInt(index),
            (Literal::Texture, _) => Self::Texture(None),
        }
    }

    #[must_use]
    pub fn default_of(spec: &ParameterSpec) -> Self {
        Self::from_literal(spec, spec.default)
    }

    #[must_use]
    pub fn fits(&self, kind: &ParameterKind) -> bool {
        matches!(
            (self, kind),
            (Self::Float(_), ParameterKind::Float)
                | (Self::Vec2(_), ParameterKind::Vec2)
                | (
                    Self::Vec3(_),
                    ParameterKind::Vec3 | ParameterKind::Color { alpha: false, .. }
                )
                | (
                    Self::Vec4(_),
                    ParameterKind::Vec4 | ParameterKind::Color { alpha: true, .. }
                )
                | (Self::Int(_), ParameterKind::Int)
                | (Self::UInt(_), ParameterKind::UInt)
                | (Self::Bool(_), ParameterKind::Bool)
                | (Self::Enum(_), ParameterKind::Enum { .. })
                | (Self::Texture(_), ParameterKind::Texture { .. })
        )
    }

    pub fn check(&self, spec: &ParameterSpec) -> Result<(), ValueError> {
        if !self.fits(&spec.kind) {
            return Err(ValueError::Mismatch {
                name: spec.name.clone(),
                expected: kind_name(&spec.kind),
                given: self.kind_name(),
            });
        }
        if let (Self::Enum(option), ParameterKind::Enum { options }) = (self, &spec.kind)
            && !options
                .iter()
                .any(|known| known.as_str() == option.as_str())
        {
            return Err(ValueError::UnknownOption {
                name: spec.name.clone(),
                option: option.to_string(),
                options: options.join(", "),
            });
        }
        if let Some(range) = spec.range
            && let Some(outside) = self
                .components()
                .find(|value| !(range.low..=range.high).contains(value))
        {
            return Err(ValueError::OutOfRange {
                name: spec.name.clone(),
                value: outside,
                low: range.low,
                high: range.high,
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Float(_) => "number",
            Self::Vec2(_) => "vec2",
            Self::Vec3(_) => "vec3",
            Self::Vec4(_) => "vec4",
            Self::Int(_) => "integer",
            Self::UInt(_) => "unsigned integer",
            Self::Bool(_) => "switch",
            Self::Enum(_) => "choice",
            Self::Choice(_) => "option",
            Self::Asset(_) => "file or text",
            Self::Texture(_) => "texture",
        }
    }

    pub fn components(&self) -> impl Iterator<Item = f32> + '_ {
        let (values, single): (&[f32], Option<f32>) = match self {
            Self::Float(value) => (&[], Some(*value)),
            Self::Vec2(value) => (value, None),
            Self::Vec3(value) => (value, None),
            Self::Vec4(value) => (value, None),
            Self::Int(value) => (&[], Some(*value as f32)),
            Self::UInt(value) => (&[], Some(*value as f32)),
            Self::Bool(_) | Self::Enum(_) | Self::Choice(_) | Self::Asset(_) | Self::Texture(_) => {
                (&[], None)
            }
        };
        values.iter().copied().chain(single)
    }

    #[must_use]
    pub fn float(&self) -> Option<f32> {
        match *self {
            Self::Float(value) => Some(value),
            Self::Int(value) => Some(value as f32),
            Self::UInt(value) => Some(value as f32),
            _ => None,
        }
    }

    #[must_use]
    pub fn three(&self) -> Option<[f32; 3]> {
        match *self {
            Self::Vec3(value) => Some(value),
            Self::Vec4([x, y, z, _]) => Some([x, y, z]),
            _ => None,
        }
    }

    #[must_use]
    pub fn bool(&self) -> Option<bool> {
        match *self {
            Self::Bool(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn option(&self) -> Option<Name> {
        match *self {
            Self::Enum(option) => Some(option),
            _ => None,
        }
    }

    #[must_use]
    pub fn to_setting(&self, texture: impl FnOnce(&Texture) -> Option<TextureBinding>) -> Setting {
        match *self {
            Self::Float(value) => Setting::Float(value),
            Self::Vec2(value) => Setting::Vec2(Vec2::from_array(value)),
            Self::Vec3(value) => Setting::Vec3(Vec3::from_array(value)),
            Self::Vec4(value) => Setting::Vec4(Vec4::from_array(value)),
            Self::Int(value) => Setting::Int(value),
            Self::UInt(value) => Setting::UInt(value),
            Self::Bool(value) => Setting::Bool(value),
            Self::Enum(option) => Setting::Enum(option.to_string()),
            Self::Choice(index) => Setting::UInt(index),
            Self::Asset(_) | Self::Texture(None) => Setting::Texture(None),
            Self::Texture(Some(ref asset)) => Setting::Texture(texture(asset)),
        }
    }

    #[must_use]
    pub fn from_setting(setting: &Setting) -> Option<Self> {
        Some(match setting {
            Setting::Float(value) => Self::Float(*value),
            Setting::Vec2(value) => Self::Vec2(value.to_array()),
            Setting::Vec3(value) => Self::Vec3(value.to_array()),
            Setting::Vec4(value) => Self::Vec4(value.to_array()),
            Setting::Int(value) => Self::Int(*value),
            Setting::UInt(value) => Self::UInt(*value),
            Setting::Bool(value) => Self::Bool(*value),
            Setting::Enum(option) => Self::Enum(Name::new(option)),
            Setting::Texture(_) => return None,
        })
    }
}

#[must_use]
pub fn kind_name(kind: &ParameterKind) -> &'static str {
    match kind {
        ParameterKind::Float => "number",
        ParameterKind::Vec2 => "vec2",
        ParameterKind::Vec3 => "vec3",
        ParameterKind::Vec4 => "vec4",
        ParameterKind::Color { alpha: false, .. } => "colour [r, g, b]",
        ParameterKind::Color { alpha: true, .. } => "colour [r, g, b, a]",
        ParameterKind::Int => "integer",
        ParameterKind::UInt => "unsigned integer",
        ParameterKind::Bool => "switch",
        ParameterKind::Enum { .. } => "choice",
        ParameterKind::Texture { .. } => "texture",
    }
}
