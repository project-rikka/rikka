use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

pub(crate) const DEFAULT_TYPE: &str = "message";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    event: String,
    data: String,
    id: Option<Arc<str>>,
}

impl Event {
    pub(crate) fn new(event: String, data: String, id: Option<Arc<str>>) -> Event {
        Event { event, data, id }
    }

    #[must_use]
    pub fn event(&self) -> &str {
        &self.event
    }

    #[must_use]
    pub fn data(&self) -> &str {
        &self.data
    }

    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.data).map_err(|error| Error::Decode {
            format: "JSON",
            source: Box::new(error),
        })
    }
}
