use std::borrow::Cow;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) enum Value {
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Text(String),
    Word(String),
    Raw(Arc<[u8]>),
    Bools(Vec<bool>),
    I32s(Vec<i32>),
    I64s(Vec<i64>),
    F32s(Vec<f32>),
    F64s(Vec<f64>),
}

impl Value {
    pub(crate) fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) | Self::Word(text) => Some(text),
            _ => None,
        }
    }

    pub(crate) fn as_raw(&self) -> Option<&Arc<[u8]>> {
        match self {
            Self::Raw(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub(crate) fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Bool(value) => Some(i64::from(*value)),
            Self::I16(value) => Some(i64::from(*value)),
            Self::I32(value) => Some(i64::from(*value)),
            Self::I64(value) => Some(*value),
            Self::F32(value) => Some(*value as i64),
            Self::F64(value) => Some(*value as i64),
            Self::Word(word) => Some(i64::from(word.as_bytes().first().copied().unwrap_or(0))),
            _ => None,
        }
    }

    pub(crate) fn as_number(&self) -> Option<f64> {
        match self {
            Self::Bool(value) => Some(f64::from(u8::from(*value))),
            Self::I16(value) => Some(f64::from(*value)),
            Self::I32(value) => Some(f64::from(*value)),
            Self::I64(value) => Some(*value as f64),
            Self::F32(value) => Some(f64::from(*value)),
            Self::F64(value) => Some(*value),
            Self::Word(_) => self.as_integer().map(|byte| byte as f64),
            _ => None,
        }
    }

    pub(crate) fn count(&self) -> usize {
        match self {
            Self::Bools(values) => values.len(),
            Self::I32s(values) => values.len(),
            Self::I64s(values) => values.len(),
            Self::F32s(values) => values.len(),
            Self::F64s(values) => values.len(),
            Self::Text(_) | Self::Raw(_) => 0,
            Self::Word(_) => 1,
            _ => 1,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Node {
    pub name: String,
    pub values: Vec<Value>,
    pub children: Vec<Node>,
}

impl Node {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            values: Vec::new(),
            children: Vec::new(),
        }
    }

    pub(crate) fn with(mut self, value: Value) -> Self {
        self.values.push(value);
        self
    }

    pub(crate) fn with_text(self, text: impl Into<String>) -> Self {
        self.with(Value::Text(text.into()))
    }

    pub(crate) fn with_i32(self, value: i32) -> Self {
        self.with(Value::I32(value))
    }

    pub(crate) fn with_i64(self, value: i64) -> Self {
        self.with(Value::I64(value))
    }

    pub(crate) fn with_f64(self, value: f64) -> Self {
        self.with(Value::F64(value))
    }

    pub(crate) fn adding(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    pub(crate) fn add(&mut self, child: Node) -> &mut Self {
        self.children.push(child);
        self
    }

    pub(crate) fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|child| child.name == name)
    }

    pub(crate) fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |child| child.name == name)
    }

    pub(crate) fn text(&self, index: usize) -> Option<&str> {
        self.values.get(index)?.as_text()
    }

    pub(crate) fn integer(&self, index: usize) -> Option<i64> {
        self.values.get(index)?.as_integer()
    }

    pub(crate) fn number(&self, index: usize) -> Option<f64> {
        self.values.get(index)?.as_number()
    }

    pub(crate) fn raw(&self, index: usize) -> Option<&Arc<[u8]>> {
        self.values.get(index)?.as_raw()
    }

    pub(crate) fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name)?.text(0)
    }

    pub(crate) fn child_integer(&self, name: &str) -> Option<i64> {
        self.child(name)?.integer(0)
    }

    pub(crate) fn child_number(&self, name: &str) -> Option<f64> {
        self.child(name)?.number(0)
    }

    pub(crate) fn doubles(&self) -> Cow<'_, [f64]> {
        if let [Value::F64s(values)] = self.values.as_slice() {
            return Cow::Borrowed(values);
        }
        Cow::Owned(self.numbers())
    }

    pub(crate) fn floats(&self) -> Cow<'_, [f32]> {
        if let [Value::F32s(values)] = self.values.as_slice() {
            return Cow::Borrowed(values);
        }
        Cow::Owned(
            self.numbers()
                .into_iter()
                .map(|value| value as f32)
                .collect(),
        )
    }

    pub(crate) fn ints(&self) -> Cow<'_, [i32]> {
        if let [Value::I32s(values)] = self.values.as_slice() {
            return Cow::Borrowed(values);
        }
        Cow::Owned(
            self.integers()
                .into_iter()
                .map(|value| value as i32)
                .collect(),
        )
    }

    pub(crate) fn longs(&self) -> Cow<'_, [i64]> {
        if let [Value::I64s(values)] = self.values.as_slice() {
            return Cow::Borrowed(values);
        }
        Cow::Owned(self.integers())
    }

    pub(crate) fn count(&self) -> usize {
        self.values.iter().map(Value::count).sum()
    }

    fn numbers(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.count());
        for value in &self.values {
            match value {
                Value::Bools(values) => {
                    out.extend(values.iter().map(|&value| f64::from(u8::from(value))))
                }
                Value::I32s(values) => out.extend(values.iter().map(|&value| f64::from(value))),
                Value::I64s(values) => out.extend(values.iter().map(|&value| value as f64)),
                Value::F32s(values) => out.extend(values.iter().map(|&value| f64::from(value))),
                Value::F64s(values) => out.extend_from_slice(values),
                scalar => out.extend(scalar.as_number()),
            }
        }
        out
    }

    fn integers(&self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.count());
        for value in &self.values {
            match value {
                Value::Bools(values) => out.extend(values.iter().map(|&value| i64::from(value))),
                Value::I32s(values) => out.extend(values.iter().map(|&value| i64::from(value))),
                Value::I64s(values) => out.extend_from_slice(values),
                Value::F32s(values) => out.extend(values.iter().map(|&value| value as i64)),
                Value::F64s(values) => out.extend(values.iter().map(|&value| value as i64)),
                scalar => out.extend(scalar.as_integer()),
            }
        }
        out
    }
}
