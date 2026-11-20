use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasContact {
    #[serde(rename = "ID")]
    id: i64,
    #[serde(rename = "Name", default)]
    name: Option<String>,
    #[serde(rename = "Blocked", default)]
    blocked: bool,
    #[serde(rename = "ReverseAlias", default)]
    reverse_alias: String,
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(rename = "RepliedEmails", default)]
    replied: i64,
    #[serde(rename = "ForwardedEmails", default)]
    forwarded: i64,
    #[serde(rename = "BlockedEmails", default)]
    blocked_emails: i64,
}

impl AliasContact {
    #[must_use]
    pub fn id(&self) -> i64 {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    #[must_use]
    pub fn reverse_alias(&self) -> &str {
        &self.reverse_alias
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn replied_emails(&self) -> i64 {
        self.replied
    }

    #[must_use]
    pub fn forwarded_emails(&self) -> i64 {
        self.forwarded
    }

    #[must_use]
    pub fn blocked_emails(&self) -> i64 {
        self.blocked_emails
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AliasContactsEnvelope {
    #[serde(rename = "Contacts", default)]
    pub(crate) contacts: Vec<AliasContact>,
    #[serde(rename = "LastID", default)]
    pub(crate) last_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateAliasContactRequest<'a> {
    #[serde(rename = "Email")]
    pub(crate) email: &'a str,
    #[serde(rename = "Name", skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BlockAliasContactRequest {
    #[serde(rename = "Blocked")]
    pub(crate) blocked: bool,
}
