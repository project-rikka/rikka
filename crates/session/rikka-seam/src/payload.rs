use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::SeamError;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Payload(Value);

impl Payload {
    pub const fn none() -> Self {
        Self(Value::Null)
    }

    pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Self, SeamError> {
        serde_json::to_value(value)
            .map(Self)
            .map_err(|error| SeamError::InvalidPayload(error.to_string()))
    }

    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, SeamError> {
        T::deserialize(&self.0).map_err(|error| SeamError::InvalidParams(error.to_string()))
    }

    pub fn is_none(&self) -> bool {
        self.0.is_null()
    }

    pub fn to_json(&self, pretty: bool) -> Result<String, SeamError> {
        let encoded = if pretty {
            serde_json::to_string_pretty(&self.0)
        } else {
            serde_json::to_string(&self.0)
        };
        encoded.map_err(|error| SeamError::InvalidPayload(error.to_string()))
    }

    pub fn from_json(text: &str) -> Result<Self, SeamError> {
        serde_json::from_str(text)
            .map(Self)
            .map_err(SeamError::invalid_params)
    }

    pub fn value(&self) -> &Value {
        &self.0
    }
}

impl From<Value> for Payload {
    fn from(value: Value) -> Self {
        Self(value)
    }
}
