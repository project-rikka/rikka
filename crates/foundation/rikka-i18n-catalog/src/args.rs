use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value<'a> {
    Text(&'a str),
    Int(i64),
    Float(f64),
}

impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Text(text) => f.write_str(text),
            Value::Int(value) => write!(f, "{value}"),
            Value::Float(value) => write!(f, "{value}"),
        }
    }
}

impl<'a> From<&'a str> for Value<'a> {
    fn from(text: &'a str) -> Self {
        Value::Text(text)
    }
}

impl<'a> From<&'a String> for Value<'a> {
    fn from(text: &'a String) -> Self {
        Value::Text(text)
    }
}

macro_rules! from_int {
    ($($ty:ty),+) => {
        $(impl From<$ty> for Value<'_> {
            fn from(value: $ty) -> Self {
                Value::Int(i64::from(value))
            }
        })+
    };
}
from_int!(i8, i16, i32, i64, u8, u16, u32);

macro_rules! from_wide_int {
    ($($ty:ty),+) => {
        $(impl From<$ty> for Value<'_> {
            #[expect(
                clippy::cast_possible_wrap,
                reason = "a count large enough to wrap is not a number anybody reads"
            )]
            fn from(value: $ty) -> Self {
                Value::Int(value as i64)
            }
        })+
    };
}
from_wide_int!(u64, usize);

impl From<f32> for Value<'_> {
    fn from(value: f32) -> Self {
        Value::Float(f64::from(value))
    }
}

impl From<f64> for Value<'_> {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Args<'a> {
    entries: Vec<(&'a str, Value<'a>)>,
}

impl<'a> Args<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(mut self, name: &'a str, value: impl Into<Value<'a>>) -> Self {
        let value = value.into();
        match self.entries.iter_mut().find(|(key, _)| *key == name) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((name, value)),
        }
        self
    }

    pub fn get(&self, name: &str) -> Option<Value<'a>> {
        self.entries
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| *value)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub const EMPTY: &'static Args<'static> = &Args {
        entries: Vec::new(),
    };
}

impl<'a, const N: usize> From<[(&'a str, Value<'a>); N]> for Args<'a> {
    fn from(entries: [(&'a str, Value<'a>); N]) -> Self {
        Self {
            entries: entries.to_vec(),
        }
    }
}
