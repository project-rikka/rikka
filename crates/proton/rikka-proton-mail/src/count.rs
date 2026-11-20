use serde::{Deserialize, Serialize};

use crate::identifier::LabelId;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LabelCount {
    #[serde(rename = "LabelID", default)]
    label_id: LabelId,
    #[serde(rename = "Total", default)]
    total: i64,
    #[serde(rename = "Unread", default)]
    unread: i64,
}

impl LabelCount {
    #[must_use]
    pub fn label_id(&self) -> &LabelId {
        &self.label_id
    }

    #[must_use]
    pub fn total(&self) -> i64 {
        self.total
    }

    #[must_use]
    pub fn unread(&self) -> i64 {
        self.unread
    }
}
