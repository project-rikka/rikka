use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::vendor::Vendor;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reasoning {
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state: Option<State>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    vendor: Vendor,
    payload: Value,
}

impl Reasoning {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Reasoning {
        Reasoning {
            text: text.into(),
            state: None,
        }
    }

    pub(crate) fn opaque(vendor: Vendor, text: impl Into<String>, payload: Value) -> Reasoning {
        Reasoning {
            text: text.into(),
            state: Some(State { vendor, payload }),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn is_replayable(&self, vendor: Vendor) -> bool {
        self.replay(vendor).is_some()
    }

    pub(crate) fn replay(&self, vendor: Vendor) -> Option<&Value> {
        self.state
            .as_ref()
            .filter(|state| state.vendor == vendor)
            .map(|state| &state.payload)
    }
}
