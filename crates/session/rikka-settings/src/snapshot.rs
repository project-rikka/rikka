use crate::domain::Domain;
use crate::value::{Value, Values};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    domain: Domain,
    values: Values,
}

impl Snapshot {
    pub(crate) fn new(domain: Domain, values: Values) -> Self {
        Self { domain, values }
    }

    #[must_use]
    pub fn values(&self) -> &Values {
        &self.values
    }

    #[must_use]
    pub fn diff(&self, next: &Self) -> Vec<Change> {
        let mut changes = Vec::new();
        for (key, value) in &next.values {
            if self.values.get(key) != Some(value) {
                changes.push(Change {
                    key: key.clone(),
                    value: Some(value.clone()),
                });
            }
        }
        for key in self.values.keys() {
            if !next.values.contains_key(key) {
                changes.push(Change {
                    key: key.clone(),
                    value: None,
                });
            }
        }
        changes
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Change {
    pub key: String,
    pub value: Option<Value>,
}
