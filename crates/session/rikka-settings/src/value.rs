use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

pub type Values = BTreeMap<String, Value>;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Table(Values),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    Bool,
    Int,
    Float,
    String,
    List,
    Table,
}

impl Value {
    #[must_use]
    pub fn value_type(&self) -> ValueType {
        match self {
            Self::Bool(_) => ValueType::Bool,
            Self::Int(_) => ValueType::Int,
            Self::Float(_) => ValueType::Float,
            Self::String(_) => ValueType::String,
            Self::List(_) => ValueType::List,
            Self::Table(_) => ValueType::Table,
        }
    }

    pub fn get<T: DeserializeOwned>(&self) -> Result<T> {
        self.to_toml()
            .try_into()
            .map_err(|source| Error::Convert(Box::new(source)))
    }

    pub fn from<T: Serialize>(value: &T) -> Result<Self> {
        let encoded =
            toml::Value::try_from(value).map_err(|source| Error::Convert(Box::new(source)))?;
        Self::from_toml(encoded).map_err(Error::Unrepresentable)
    }

    #[must_use]
    pub(crate) fn to_toml(&self) -> toml::Value {
        match self {
            Self::Bool(inner) => toml::Value::Boolean(*inner),
            Self::Int(inner) => toml::Value::Integer(*inner),
            Self::Float(inner) => toml::Value::Float(*inner),
            Self::String(inner) => toml::Value::String(inner.clone()),
            Self::List(inner) => toml::Value::Array(inner.iter().map(Self::to_toml).collect()),
            Self::Table(inner) => toml::Value::Table(
                inner
                    .iter()
                    .map(|(name, value)| (name.clone(), value.to_toml()))
                    .collect(),
            ),
        }
    }

    pub(crate) fn from_toml(value: toml::Value) -> std::result::Result<Self, String> {
        Ok(match value {
            toml::Value::Boolean(inner) => Self::Bool(inner),
            toml::Value::Integer(inner) => Self::Int(inner),
            toml::Value::Float(inner) => Self::Float(inner),
            toml::Value::String(inner) => Self::String(inner),
            toml::Value::Array(inner) => Self::List(
                inner
                    .into_iter()
                    .map(Self::from_toml)
                    .collect::<std::result::Result<_, _>>()?,
            ),
            toml::Value::Table(inner) => Self::Table(
                inner
                    .into_iter()
                    .map(|(name, value)| Self::from_toml(value).map(|value| (name, value)))
                    .collect::<std::result::Result<_, _>>()?,
            ),
            toml::Value::Datetime(inner) => {
                return Err(format!("a settings value cannot be a datetime ({inner})"));
            }
        })
    }

    #[must_use]
    pub fn from_literal(text: &str) -> Option<Self> {
        Self::from_toml(text.parse::<toml::Value>().ok()?).ok()
    }

    #[must_use]
    pub(crate) fn coerced(self, target: ValueType) -> Self {
        match (self, target) {
            (Self::Int(inner), ValueType::Float) => Self::Float(inner as f64),
            (inner, _) => inner,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.to_toml(), f)
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Bool => "a true/false",
            Self::Int => "a whole number",
            Self::Float => "a number",
            Self::String => "some text",
            Self::List => "a list",
            Self::Table => "a table",
        })
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Self::Int(value.into())
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Self::List(value)
    }
}

impl From<Values> for Value {
    fn from(value: Values) -> Self {
        Self::Table(value)
    }
}
