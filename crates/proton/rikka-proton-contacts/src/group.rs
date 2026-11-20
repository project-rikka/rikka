use serde::{Deserialize, Serialize};

use crate::identifier::GroupId;

pub(crate) const CONTACT_GROUP: i32 = 2;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactGroup {
    #[serde(rename = "ID", default)]
    id: GroupId,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Color", default)]
    color: String,
    #[serde(rename = "Path", default)]
    path: String,
    #[serde(rename = "Order", default)]
    order: i32,
    #[serde(rename = "Display", default, with = "rikka_proton_api::flag")]
    display: bool,
    #[serde(rename = "Notify", default, with = "rikka_proton_api::flag")]
    notify: bool,
    #[serde(rename = "Type", default)]
    kind: i32,
}

impl ContactGroup {
    #[must_use]
    pub fn id(&self) -> &GroupId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn color(&self) -> &str {
        &self.color
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn order(&self) -> i32 {
        self.order
    }

    #[must_use]
    pub fn is_displayed(&self) -> bool {
        self.display
    }

    #[must_use]
    pub fn notifies(&self) -> bool {
        self.notify
    }

    #[must_use]
    pub fn is_contact_group(&self) -> bool {
        self.kind == CONTACT_GROUP
    }
}
