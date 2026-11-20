use jiff::Timestamp;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::identifier::{KeyId, UserId};
use crate::user_key::UserKey;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "ID")]
    id: UserId,
    #[serde(rename = "Name", default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(
        rename = "DisplayName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    display_name: Option<String>,
    #[serde(rename = "Email", default, skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(rename = "Keys", default)]
    keys: Vec<UserKey>,
    #[serde(rename = "UsedSpace", default)]
    used_space: u64,
    #[serde(rename = "MaxSpace", default)]
    max_space: u64,
    #[serde(rename = "MaxUpload", default)]
    max_upload: u64,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(rename = "Credit", default)]
    credit: i64,
    #[serde(rename = "Currency", default)]
    currency: String,
    #[serde(rename = "Type", default)]
    kind: i32,
    #[serde(rename = "Role", default)]
    role: i32,
    #[serde(rename = "Private", default, with = "rikka_proton_api::flag")]
    private: bool,
    #[serde(rename = "Subscribed", default)]
    subscribed: i32,
    #[serde(rename = "Services", default)]
    services: i32,
    #[serde(rename = "Delinquent", default)]
    delinquent: i32,
    #[serde(rename = "Flags", default, skip_serializing_if = "Option::is_none")]
    flags: Option<BTreeMap<String, bool>>,
}

impl User {
    #[must_use]
    pub fn id(&self) -> &UserId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    #[must_use]
    pub fn keys(&self) -> &[UserKey] {
        &self.keys
    }

    #[must_use]
    pub fn key(&self, id: &KeyId) -> Option<&UserKey> {
        self.keys.iter().find(|key| key.id() == id)
    }

    #[must_use]
    pub fn primary_key(&self) -> Option<&UserKey> {
        self.keys
            .iter()
            .find(|key| key.is_primary())
            .or_else(|| self.keys.first())
    }

    #[must_use]
    pub fn used_space(&self) -> u64 {
        self.used_space
    }

    #[must_use]
    pub fn max_space(&self) -> u64 {
        self.max_space
    }

    #[must_use]
    pub fn max_upload(&self) -> u64 {
        self.max_upload
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn credit(&self) -> i64 {
        self.credit
    }

    #[must_use]
    pub fn currency(&self) -> &str {
        &self.currency
    }

    #[must_use]
    pub fn kind(&self) -> i32 {
        self.kind
    }

    #[must_use]
    pub fn role(&self) -> i32 {
        self.role
    }

    #[must_use]
    pub fn is_private(&self) -> bool {
        self.private
    }

    #[must_use]
    pub fn subscribed(&self) -> i32 {
        self.subscribed
    }

    #[must_use]
    pub fn services(&self) -> i32 {
        self.services
    }

    #[must_use]
    pub fn delinquent(&self) -> i32 {
        self.delinquent
    }

    #[must_use]
    pub fn flag(&self, name: &str) -> Option<bool> {
        self.flags
            .as_ref()
            .and_then(|flags| flags.get(name))
            .copied()
    }
}
