use std::fmt;

use crate::key::Key;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Id {
    Code(Key),
    Text(String),
}

impl Id {
    pub fn code(code: &[u8; 4]) -> Self {
        Id::Code(Key(*code))
    }

    pub fn text(text: impl Into<String>) -> Self {
        Id::Text(text.into())
    }

    pub fn as_str(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Id::Code(key) => key.as_str(),
            Id::Text(text) => std::borrow::Cow::Borrowed(text),
        }
    }

    pub fn is(&self, name: &str) -> bool {
        match self {
            Id::Code(key) => key.0.as_slice() == name.as_bytes(),
            Id::Text(text) => text == name,
        }
    }
}

impl From<&str> for Id {
    fn from(text: &str) -> Self {
        const LONG_FOUR: [&str; 4] = ["warp", "time", "hold", "list"];
        match <[u8; 4]>::try_from(text.as_bytes()) {
            Ok(code) if !LONG_FOUR.contains(&text) => Id::Code(Key(code)),
            _ => Id::Text(text.to_owned()),
        }
    }
}

impl From<String> for Id {
    fn from(text: String) -> Self {
        Id::from(text.as_str())
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Id::Code(key) => write!(f, "'{key}'"),
            Id::Text(text) => write!(f, "{text:?}"),
        }
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Unit(pub Key);

impl Unit {
    pub const ANGLE: Unit = Unit(Key(*b"#Ang"));
    pub const DENSITY: Unit = Unit(Key(*b"#Rsl"));
    pub const DISTANCE: Unit = Unit(Key(*b"#Rlt"));
    pub const NONE: Unit = Unit(Key(*b"#Nne"));
    pub const PERCENT: Unit = Unit(Key(*b"#Prc"));
    pub const PIXELS: Unit = Unit(Key(*b"#Pxl"));
    pub const POINTS: Unit = Unit(Key(*b"#Pnt"));
    pub const MILLIMETERS: Unit = Unit(Key(*b"#Mlm"));
    pub const CENTIMETERS: Unit = Unit(Key(*b"RrCm"));
    pub const INCHES: Unit = Unit(Key(*b"RrIn"));
    pub const PICAS: Unit = Unit(Key(*b"RrPi"));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    pub name: String,
    pub id: Id,
}

impl Class {
    pub fn new(id: impl Into<Id>) -> Self {
        Self {
            name: String::new(),
            id: id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reference {
    Property { class: Class, key: Id },
    Class(Class),
    Enumerated { class: Class, kind: Id, value: Id },
    Offset { class: Class, value: u32 },
    Identifier(i32),
    Index(i32),
    Name { class: Class, value: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectArray {
    pub count: u32,
    pub name: String,
    pub class: Id,
    pub items: Vec<(Id, Value)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Reference(Vec<Reference>),
    Descriptor(Descriptor),
    GlobalObject(Descriptor),
    List(Vec<Value>),
    Double(f64),
    UnitDouble(Unit, f64),
    UnitFloats(Unit, Vec<f64>),
    String(String),
    Enum(Id, Id),
    Integer(i32),
    LargeInteger(i64),
    Bool(bool),
    Class(Class),
    GlobalClass(Class),
    Alias(Vec<u8>),
    RawData(Vec<u8>),
    Path(Vec<u8>),
    ObjectArray(ObjectArray),
}

impl Value {
    pub fn number(&self) -> Option<f64> {
        match self {
            Value::Double(value) | Value::UnitDouble(_, value) => Some(*value),
            Value::Integer(value) => Some(f64::from(*value)),
            Value::LargeInteger(value) => Some(*value as f64),
            Value::UnitFloats(_, values) if values.len() == 1 => Some(values[0]),
            _ => None,
        }
    }

    pub fn unit(&self) -> Option<Unit> {
        match self {
            Value::UnitDouble(unit, _) | Value::UnitFloats(unit, _) => Some(*unit),
            _ => None,
        }
    }

    pub fn as_descriptor(&self) -> Option<&Descriptor> {
        match self {
            Value::Descriptor(descriptor) | Value::GlobalObject(descriptor) => Some(descriptor),
            _ => None,
        }
    }

    pub fn as_descriptor_mut(&mut self) -> Option<&mut Descriptor> {
        match self {
            Value::Descriptor(descriptor) | Value::GlobalObject(descriptor) => Some(descriptor),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_enum(&self) -> Option<(&Id, &Id)> {
        match self {
            Value::Enum(kind, value) => Some((kind, value)),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::RawData(bytes) | Value::Alias(bytes) | Value::Path(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn integer(&self) -> Option<i64> {
        match self {
            Value::Integer(value) => Some(i64::from(*value)),
            Value::LargeInteger(value) => Some(*value),
            Value::Double(value) | Value::UnitDouble(_, value) => Some(value.round() as i64),
            _ => None,
        }
    }
}

impl From<Descriptor> for Value {
    fn from(descriptor: Descriptor) -> Self {
        Value::Descriptor(descriptor)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Double(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Integer(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_owned())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<Vec<Value>> for Value {
    fn from(items: Vec<Value>) -> Self {
        Value::List(items)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Descriptor {
    pub name: String,
    pub class: Id,
    pub items: Vec<(Id, Value)>,
}

impl Default for Descriptor {
    fn default() -> Self {
        Self::new("null")
    }
}

impl Descriptor {
    pub fn new(class: impl Into<Id>) -> Self {
        Self {
            name: String::new(),
            class: class.into(),
            items: Vec::new(),
        }
    }

    #[must_use]
    pub fn with(mut self, key: impl Into<Id>, value: impl Into<Value>) -> Self {
        self.set(key, value);
        self
    }

    pub fn set(&mut self, key: impl Into<Id>, value: impl Into<Value>) {
        let key = key.into();
        let value = value.into();
        let position = self.items.iter().position(|(id, _)| id.is(&key.as_str()));
        match position {
            Some(index) => self.items[index].1 = value,
            None => self.items.push((key, value)),
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let index = self.items.iter().position(|(id, _)| id.is(key))?;
        Some(self.items.remove(index).1)
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.items
            .iter()
            .find(|(id, _)| id.is(key))
            .map(|(_, value)| value)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.items
            .iter_mut()
            .find(|(id, _)| id.is(key))
            .map(|(_, value)| value)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn number(&self, key: &str) -> Option<f64> {
        self.get(key)?.number()
    }

    pub fn integer(&self, key: &str) -> Option<i64> {
        self.get(key)?.integer()
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.get(key)?.as_bool()
    }

    pub fn string(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    pub fn descriptor(&self, key: &str) -> Option<&Descriptor> {
        self.get(key)?.as_descriptor()
    }

    pub fn descriptor_mut(&mut self, key: &str) -> Option<&mut Descriptor> {
        self.get_mut(key)?.as_descriptor_mut()
    }

    pub fn list(&self, key: &str) -> Option<&[Value]> {
        self.get(key)?.as_list()
    }

    pub fn enumeration(&self, key: &str) -> Option<std::borrow::Cow<'_, str>> {
        self.get(key)?.as_enum().map(|(_, value)| value.as_str())
    }

    pub fn bytes(&self, key: &str) -> Option<&[u8]> {
        self.get(key)?.as_bytes()
    }

    pub fn unit_value(&self, key: &str) -> Option<(Unit, f64)> {
        match self.get(key)? {
            Value::UnitDouble(unit, value) => Some((*unit, *value)),
            Value::UnitFloats(unit, values) if !values.is_empty() => Some((*unit, values[0])),
            other => other.number().map(|value| (Unit::NONE, value)),
        }
    }
}
