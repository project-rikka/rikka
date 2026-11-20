use super::error::Error;
use super::{decode, encode};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Unsigned(u64),
    Negative(u64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Tag(u64, Box<Value>),
    Bool(bool),
    Null,
    Undefined,
    Float(f64),
}

impl Value {
    #[must_use]
    pub fn map<K, V>(entries: impl IntoIterator<Item = (K, V)>) -> Value
    where
        K: Into<Value>,
        V: Into<Value>,
    {
        Value::Map(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        )
    }

    #[must_use]
    pub fn array<V: Into<Value>>(items: impl IntoIterator<Item = V>) -> Value {
        Value::Array(items.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub fn bytes(bytes: impl Into<Vec<u8>>) -> Value {
        Value::Bytes(bytes.into())
    }

    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        encode::write(self, &mut out);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Value, Error> {
        let (value, used) = decode::read(bytes)?;
        match bytes.len() - used {
            0 => Ok(value),
            left => Err(Error::Trailing(left)),
        }
    }

    pub fn decode_prefix(bytes: &[u8]) -> Result<(Value, usize), Error> {
        decode::read(bytes)
    }

    #[must_use]
    pub fn integer(&self) -> Option<i128> {
        match *self {
            Value::Unsigned(n) => Some(i128::from(n)),
            Value::Negative(n) => Some(-1 - i128::from(n)),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_unsigned(&self) -> Option<u64> {
        match *self {
            Value::Unsigned(n) => Some(n),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        self.integer().and_then(|n| i64::try_from(n).ok())
    }

    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_map(&self) -> Option<&[(Value, Value)]> {
        match self {
            Value::Map(entries) => Some(entries),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Bool(flag) => Some(flag),
            _ => None,
        }
    }

    #[must_use]
    pub fn get(&self, key: impl Into<Value>) -> Option<&Value> {
        let key = key.into();
        self.as_map()?
            .iter()
            .find(|(candidate, _)| *candidate == key)
            .map(|(_, value)| value)
    }
}

impl From<u64> for Value {
    fn from(n: u64) -> Value {
        Value::Unsigned(n)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Value {
        match u64::try_from(n) {
            Ok(n) => Value::Unsigned(n),
            Err(_) => Value::Negative(!n as u64),
        }
    }
}

macro_rules! widened {
    ($($narrow:ty => $wide:ty),*) => {
        $(impl From<$narrow> for Value {
            fn from(n: $narrow) -> Value {
                Value::from(<$wide>::from(n))
            }
        })*
    };
}

widened!(u8 => u64, u16 => u64, u32 => u64, i8 => i64, i16 => i64, i32 => i64);

impl From<usize> for Value {
    fn from(n: usize) -> Value {
        Value::Unsigned(n as u64)
    }
}

impl From<bool> for Value {
    fn from(flag: bool) -> Value {
        Value::Bool(flag)
    }
}

impl From<&str> for Value {
    fn from(text: &str) -> Value {
        Value::Text(text.to_owned())
    }
}

impl From<String> for Value {
    fn from(text: String) -> Value {
        Value::Text(text)
    }
}

impl From<&[u8]> for Value {
    fn from(bytes: &[u8]) -> Value {
        Value::Bytes(bytes.to_vec())
    }
}

impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Value {
        Value::Bytes(bytes)
    }
}

impl<const N: usize> From<[u8; N]> for Value {
    fn from(bytes: [u8; N]) -> Value {
        Value::Bytes(bytes.to_vec())
    }
}

impl From<Vec<Value>> for Value {
    fn from(items: Vec<Value>) -> Value {
        Value::Array(items)
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Value {
        Value::Float(n)
    }
}
