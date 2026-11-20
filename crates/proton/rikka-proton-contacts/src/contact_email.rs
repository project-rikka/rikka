use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::identifier::{ContactEmailId, ContactId, GroupId};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactEmail {
    #[serde(rename = "ID", default)]
    id: ContactEmailId,
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Type", default)]
    types: Vec<String>,
    #[serde(rename = "Defaults", default, with = "rikka_proton_api::flag")]
    defaults: bool,
    #[serde(rename = "Order", default)]
    order: i32,
    #[serde(rename = "ContactID", default)]
    contact_id: ContactId,
    #[serde(rename = "LabelIDs", default)]
    groups: Vec<GroupId>,
    #[serde(
        rename = "LastUsedTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    last_used_time: Timestamp,
}

impl ContactEmail {
    #[must_use]
    pub fn id(&self) -> &ContactEmailId {
        &self.id
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn types(&self) -> &[String] {
        &self.types
    }

    #[must_use]
    pub fn uses_defaults(&self) -> bool {
        self.defaults
    }

    #[must_use]
    pub fn order(&self) -> i32 {
        self.order
    }

    #[must_use]
    pub fn contact_id(&self) -> &ContactId {
        &self.contact_id
    }

    #[must_use]
    pub fn groups(&self) -> &[GroupId] {
        &self.groups
    }

    #[must_use]
    pub fn last_used_time(&self) -> Timestamp {
        self.last_used_time
    }
}
