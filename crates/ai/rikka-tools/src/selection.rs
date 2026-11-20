use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Selection {
    #[serde(default, rename = "chosen")]
    labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wrote: Option<String>,
}

impl Selection {
    #[must_use]
    pub fn new(labels: Vec<String>, wrote: Option<String>) -> Selection {
        Selection {
            labels,
            wrote: wrote.filter(|text| !text.trim().is_empty()),
        }
    }

    #[must_use]
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.wrote.as_deref()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty() && self.wrote.is_none()
    }
}
