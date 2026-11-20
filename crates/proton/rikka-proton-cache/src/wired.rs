use std::fmt;
use std::ops::Deref;

use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

#[derive(Clone)]
pub struct Wired<T> {
    wire: Value,
    value: T,
}

impl<T: DeserializeOwned> Wired<T> {
    pub fn from_wire(wire: Value) -> serde_json::Result<Wired<T>> {
        let value = T::deserialize(&wire)?;
        Ok(Wired { wire, value })
    }
}

impl<T> Wired<T> {
    #[must_use]
    pub fn wire(&self) -> &Value {
        &self.wire
    }

    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }

    #[must_use]
    pub fn into_parts(self) -> (Value, T) {
        (self.wire, self.value)
    }
}

impl<T> Deref for Wired<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> Serialize for Wired<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.wire.serialize(serializer)
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Wired<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Wired<T>, D::Error> {
        let wire = Value::deserialize(deserializer)?;
        Wired::from_wire(wire).map_err(D::Error::custom)
    }
}

impl<T: fmt::Debug> fmt::Debug for Wired<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(formatter)
    }
}
