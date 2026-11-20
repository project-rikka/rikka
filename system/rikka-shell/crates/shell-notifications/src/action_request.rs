use rikka_seam::{Payload, Request};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActionRequest {
    capability: String,
    method: String,
    params: Payload,
}

impl ActionRequest {
    #[must_use]
    pub fn new(capability: impl Into<String>, method: impl Into<String>) -> ActionRequest {
        ActionRequest {
            capability: capability.into(),
            method: method.into(),
            params: Payload::none(),
        }
    }

    #[must_use]
    pub fn encode<T: Serialize + ?Sized>(mut self, value: &T) -> ActionRequest {
        match Payload::encode(value) {
            Ok(params) => self.params = params,
            Err(error) => tracing::warn!(
                "calling {}/{} without parameters: {error}",
                self.capability,
                self.method
            ),
        }
        self
    }

    #[must_use]
    pub(crate) fn request(&self) -> Request {
        Request::new(self.capability.as_str(), self.method.as_str()).params(self.params.clone())
    }
}
