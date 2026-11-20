use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::filter::simple::SimpleFilter;
use crate::identifier::FilterId;

pub(crate) const ENABLED: i32 = 1;
pub(crate) const DISABLED: i32 = 0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Filter {
    #[serde(rename = "ID", default)]
    id: FilterId,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Status", default)]
    status: i32,
    #[serde(rename = "Priority", default)]
    priority: i64,
    #[serde(rename = "Version", default)]
    version: i32,
    #[serde(rename = "Sieve", default)]
    sieve: String,
    #[serde(rename = "Simple", default, skip_serializing_if = "Option::is_none")]
    simple: Option<Value>,
    #[serde(rename = "Tree", default, skip_serializing_if = "Option::is_none")]
    tree: Option<Value>,
}

impl Filter {
    #[must_use]
    pub fn id(&self) -> &FilterId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.status == ENABLED
    }

    #[must_use]
    pub fn priority(&self) -> i64 {
        self.priority
    }

    #[must_use]
    pub fn version(&self) -> i32 {
        self.version
    }

    #[must_use]
    pub fn sieve(&self) -> &str {
        &self.sieve
    }

    #[must_use]
    pub fn tree(&self) -> Option<&Value> {
        self.tree.as_ref()
    }

    #[must_use]
    pub fn simple(&self) -> Option<SimpleFilter> {
        self.simple
            .as_ref()
            .and_then(SimpleFilter::from_wire)
            .or_else(|| SimpleFilter::from_sieve(&self.sieve))
    }

    #[must_use]
    pub fn is_advanced(&self) -> bool {
        self.simple().is_none()
    }
}
