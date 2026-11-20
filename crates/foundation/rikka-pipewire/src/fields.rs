use libspa::pod::deserialize::PodDeserializer;
use libspa::pod::{Pod, Property, Value, ValueArray};
use libspa::utils::Id;

#[derive(Debug, Clone, PartialEq)]
pub struct Fields(Vec<Property>);

impl Fields {
    #[must_use]
    pub fn parse(param: &Pod) -> Option<Fields> {
        let (_, value) = PodDeserializer::deserialize_any_from(param.as_bytes()).ok()?;
        match value {
            Value::Object(object) => Some(Fields(object.properties)),
            _ => None,
        }
    }

    #[must_use]
    pub fn get(&self, key: u32) -> Option<&Value> {
        self.0
            .iter()
            .find(|property| property.key == key)
            .map(|property| &property.value)
    }

    #[must_use]
    pub fn int(&self, key: u32) -> Option<i32> {
        match self.get(key)? {
            Value::Int(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn long(&self, key: u32) -> Option<i64> {
        match self.get(key)? {
            Value::Long(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn id(&self, key: u32) -> Option<u32> {
        match self.get(key)? {
            Value::Id(Id(value)) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn bool(&self, key: u32) -> Option<bool> {
        match self.get(key)? {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn string(&self, key: u32) -> Option<&str> {
        match self.get(key)? {
            Value::String(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn floats(&self, key: u32) -> Option<&[f32]> {
        match self.get(key)? {
            Value::ValueArray(ValueArray::Float(values)) => Some(values),
            _ => None,
        }
    }

    #[must_use]
    pub fn ints(&self, key: u32) -> Option<&[i32]> {
        match self.get(key)? {
            Value::ValueArray(ValueArray::Int(values)) => Some(values),
            _ => None,
        }
    }
}
