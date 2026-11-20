use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::vendor::Vendor;

#[derive(Debug, Clone, PartialEq)]
pub struct ToolUse {
    id: String,
    name: String,
    input: Value,
    signature: Option<Signature>,
}

#[derive(Debug, Clone, PartialEq)]
struct Signature {
    vendor: Vendor,
    payload: String,
}

impl ToolUse {
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, input: Value) -> ToolUse {
        ToolUse {
            id: id.into(),
            name: name.into(),
            input,
            signature: None,
        }
    }

    pub(crate) fn signed(mut self, vendor: Vendor, payload: impl Into<String>) -> ToolUse {
        self.signature = Some(Signature {
            vendor,
            payload: payload.into(),
        });
        self
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn input(&self) -> &Value {
        &self.input
    }

    #[must_use]
    pub fn is_signed(&self, vendor: Vendor) -> bool {
        self.signature(vendor).is_some()
    }

    pub(crate) fn signature(&self, vendor: Vendor) -> Option<&str> {
        self.signature
            .as_ref()
            .filter(|signature| signature.vendor == vendor)
            .map(|signature| signature.payload.as_str())
    }

    pub fn arguments<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_value(self.input.clone()).map_err(|error| Error::Format(Box::new(error)))
    }
}
