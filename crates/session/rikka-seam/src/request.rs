use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::capability::{CapabilityId, Version};
use crate::error::SeamError;
use crate::method::Method;
use crate::payload::Payload;

#[derive(Clone, Debug)]
pub struct Request {
    capability: CapabilityId,
    method: Method,
    payload: Payload,
    require: Option<Version>,
    error: Option<SeamError>,
}

impl Request {
    pub fn new(capability: impl Into<CapabilityId>, method: impl Into<Method>) -> Self {
        Self {
            capability: capability.into(),
            method: method.into(),
            payload: Payload::none(),
            require: None,
            error: None,
        }
    }

    #[must_use]
    pub fn params(mut self, payload: impl Into<Payload>) -> Self {
        self.payload = payload.into();
        self
    }

    #[must_use]
    pub fn encode<T: Serialize + ?Sized>(mut self, value: &T) -> Self {
        match Payload::encode(value) {
            Ok(payload) => self.payload = payload,
            Err(error) => {
                self.error.get_or_insert(error);
            }
        }
        self
    }

    #[must_use]
    pub fn require(mut self, version: Version) -> Self {
        self.require = Some(version);
        self
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub fn method(&self) -> &Method {
        &self.method
    }

    pub fn required_version(&self) -> Option<Version> {
        self.require
    }

    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    pub fn into_payload(self) -> Payload {
        self.payload
    }

    pub fn into_parts(self) -> (CapabilityId, Method, Payload, Option<Version>) {
        (self.capability, self.method, self.payload, self.require)
    }

    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, SeamError> {
        self.payload.decode()
    }

    pub(crate) fn take_error(&mut self) -> Option<SeamError> {
        self.error.take()
    }
}
