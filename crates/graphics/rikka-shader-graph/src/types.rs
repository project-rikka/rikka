use std::collections::BTreeSet;
use std::fmt;
use std::sync::Mutex;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(&'static str);

static SYMBOLS: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());

impl Symbol {
    pub const fn fixed(name: &'static str) -> Self {
        Self(name)
    }

    pub fn new(name: &str) -> Self {
        let mut known = SYMBOLS.lock().unwrap_or_else(|poison| poison.into_inner());
        if let Some(found) = known.get(name) {
            return Self(found);
        }
        let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
        known.insert(leaked);
        Self(leaked)
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Serialize for Symbol {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for Symbol {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::new(&String::deserialize(deserializer)?))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Float,
    Float2,
    Float3,
    Float4,
    Texture,
    Shader,
    Image,
    Record(Symbol),
}

impl ValueType {
    pub fn vector(width: usize) -> Option<Self> {
        match width {
            1 => Some(Self::Float),
            2 => Some(Self::Float2),
            3 => Some(Self::Float3),
            4 => Some(Self::Float4),
            _ => None,
        }
    }

    pub fn width(self) -> Option<usize> {
        match self {
            Self::Float => Some(1),
            Self::Float2 => Some(2),
            Self::Float3 => Some(3),
            Self::Float4 => Some(4),
            Self::Texture | Self::Shader | Self::Image | Self::Record(_) => None,
        }
    }

    pub fn is_number(self) -> bool {
        self.width().is_some()
    }

    pub(crate) fn slang(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Float2 => "float2",
            Self::Float3 => "float3",
            Self::Float4 => "float4",
            Self::Texture => "RikkaTexture",
            Self::Shader => "RikkaShade",
            Self::Image | Self::Record(_) => {
                unreachable!("images and records are never shader locals")
            }
        }
    }

    pub(crate) fn coerce_to(self, target: Self) -> Result<Coercion, CoercionError> {
        match (self, target) {
            (Self::Image, Self::Image) => return Ok(Coercion::Same),
            (Self::Image, to) if to.is_number() => {
                return Ok(Coercion::Load(to.width().expect("a number")));
            }
            (from, Self::Image) if from.is_number() => return Ok(Coercion::Store),
            _ => {}
        }
        match (self.width(), target.width()) {
            (None, None) if self == target => Ok(Coercion::Same),
            (Some(from), Some(to)) if from == to => Ok(Coercion::Same),
            (Some(1), Some(to)) => Ok(Coercion::Splat(to)),
            (Some(from), Some(to)) if from > to => Ok(Coercion::Truncate(to)),
            (Some(_), Some(_)) => Err(CoercionError::Widen),
            _ => Err(CoercionError::Mismatch),
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Float => formatter.write_str("float"),
            Self::Float2 => formatter.write_str("float2"),
            Self::Float3 => formatter.write_str("float3"),
            Self::Float4 => formatter.write_str("float4"),
            Self::Texture => formatter.write_str("texture"),
            Self::Shader => formatter.write_str("shader"),
            Self::Image => formatter.write_str("image"),
            Self::Record(kind) => kind.fmt(formatter),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Coercion {
    Same,
    Splat(usize),
    Truncate(usize),
    Load(usize),
    Store,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CoercionError {
    Widen,
    Mismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortType {
    Value(ValueType),
    Dynamic,
}

impl fmt::Display for PortType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(ty) => ty.fmt(formatter),
            Self::Dynamic => formatter.write_str("any float"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constant {
    width: u8,
    values: [f32; 4],
}

impl Constant {
    pub const fn scalar(value: f32) -> Self {
        Self {
            width: 1,
            values: [value, 0.0, 0.0, 0.0],
        }
    }

    pub const fn vec2(x: f32, y: f32) -> Self {
        Self {
            width: 2,
            values: [x, y, 0.0, 0.0],
        }
    }

    pub const fn vec3(x: f32, y: f32, z: f32) -> Self {
        Self {
            width: 3,
            values: [x, y, z, 0.0],
        }
    }

    pub const fn vec4(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self {
            width: 4,
            values: [x, y, z, w],
        }
    }

    pub fn from_slice(values: &[f32]) -> Option<Self> {
        if values.is_empty() || values.len() > 4 {
            return None;
        }
        let mut stored = [0.0; 4];
        stored[..values.len()].copy_from_slice(values);
        Some(Self {
            width: values.len() as u8,
            values: stored,
        })
    }

    pub fn values(&self) -> &[f32] {
        &self.values[..usize::from(self.width)]
    }

    pub fn width(&self) -> usize {
        usize::from(self.width)
    }

    pub fn ty(&self) -> ValueType {
        ValueType::vector(self.width()).expect("a constant holds one to four values")
    }

    pub fn is_finite(&self) -> bool {
        self.values().iter().all(|value| value.is_finite())
    }

    pub fn component(&self, index: usize) -> f32 {
        self.values[index.min(self.width() - 1)]
    }

    pub fn resized(&self, width: usize) -> Self {
        let mut values = [0.0; 4];
        for (index, value) in values.iter_mut().enumerate().take(width) {
            *value = if self.width == 1 {
                self.values[0]
            } else {
                self.values[index]
            };
        }
        Self {
            width: width as u8,
            values,
        }
    }
}

impl Serialize for Constant {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.values().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Constant {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let values = Vec::<f32>::deserialize(deserializer)?;
        Self::from_slice(&values).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "a constant holds one to four values, not {}",
                values.len()
            ))
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SocketValue {
    Number(Constant),
    Asset(String),
}

impl SocketValue {
    pub fn number(&self) -> Option<Constant> {
        match self {
            Self::Number(constant) => Some(*constant),
            Self::Asset(_) => None,
        }
    }

    pub fn asset(&self) -> Option<&str> {
        match self {
            Self::Asset(path) => Some(path),
            Self::Number(_) => None,
        }
    }
}

impl From<Constant> for SocketValue {
    fn from(constant: Constant) -> Self {
        Self::Number(constant)
    }
}

impl From<f32> for SocketValue {
    fn from(value: f32) -> Self {
        Self::Number(Constant::scalar(value))
    }
}

impl From<[f32; 2]> for SocketValue {
    fn from([x, y]: [f32; 2]) -> Self {
        Self::Number(Constant::vec2(x, y))
    }
}

impl From<[f32; 3]> for SocketValue {
    fn from([x, y, z]: [f32; 3]) -> Self {
        Self::Number(Constant::vec3(x, y, z))
    }
}

impl From<[f32; 4]> for SocketValue {
    fn from([x, y, z, w]: [f32; 4]) -> Self {
        Self::Number(Constant::vec4(x, y, z, w))
    }
}

impl From<&str> for SocketValue {
    fn from(path: &str) -> Self {
        Self::Asset(path.to_owned())
    }
}
