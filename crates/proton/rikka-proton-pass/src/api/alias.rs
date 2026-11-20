use serde::{Deserialize, Serialize};

use crate::api::item::CreateItemRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasMailbox {
    #[serde(rename = "ID")]
    id: i64,
    #[serde(rename = "Email", default)]
    email: String,
}

impl AliasMailbox {
    #[must_use]
    pub fn id(&self) -> i64 {
        self.id
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasSuffix {
    #[serde(rename = "Suffix")]
    suffix: String,
    #[serde(rename = "SignedSuffix")]
    signed_suffix: String,
    #[serde(rename = "IsCustom", default)]
    custom: bool,
    #[serde(rename = "IsPremium", default)]
    premium: bool,
    #[serde(rename = "Domain", default)]
    domain: String,
}

impl AliasSuffix {
    #[must_use]
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    #[must_use]
    pub fn signed_suffix(&self) -> &str {
        &self.signed_suffix
    }

    #[must_use]
    pub fn is_custom(&self) -> bool {
        self.custom
    }

    #[must_use]
    pub fn is_premium(&self) -> bool {
        self.premium
    }

    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasOptions {
    #[serde(rename = "Suffixes", default)]
    suffixes: Vec<AliasSuffix>,
    #[serde(rename = "Mailboxes", default)]
    mailboxes: Vec<AliasMailbox>,
}

impl AliasOptions {
    #[must_use]
    pub fn suffixes(&self) -> &[AliasSuffix] {
        &self.suffixes
    }

    #[must_use]
    pub fn mailboxes(&self) -> &[AliasMailbox] {
        &self.mailboxes
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AliasStats {
    #[serde(rename = "ForwardedEmails", default)]
    forwarded: i64,
    #[serde(rename = "RepliedEmails", default)]
    replied: i64,
    #[serde(rename = "BlockedEmails", default)]
    blocked: i64,
}

impl AliasStats {
    #[must_use]
    pub fn forwarded(&self) -> i64 {
        self.forwarded
    }

    #[must_use]
    pub fn replied(&self) -> i64 {
        self.replied
    }

    #[must_use]
    pub fn blocked(&self) -> i64 {
        self.blocked
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasDetails {
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "Modify", default)]
    modify: bool,
    #[serde(rename = "Mailboxes", default)]
    mailboxes: Vec<AliasMailbox>,
    #[serde(rename = "AvailableMailboxes", default)]
    available_mailboxes: Vec<AliasMailbox>,
    #[serde(rename = "Stats", default)]
    stats: Option<AliasStats>,
    #[serde(rename = "Name", default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "DisplayName", default)]
    display_name: String,
    #[serde(rename = "Note", default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

impl AliasDetails {
    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn can_modify(&self) -> bool {
        self.modify
    }

    #[must_use]
    pub fn mailboxes(&self) -> &[AliasMailbox] {
        &self.mailboxes
    }

    #[must_use]
    pub fn available_mailboxes(&self) -> &[AliasMailbox] {
        &self.available_mailboxes
    }

    #[must_use]
    pub fn stats(&self) -> Option<AliasStats> {
        self.stats
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateAliasRequest {
    #[serde(rename = "Prefix")]
    pub(crate) prefix: String,
    #[serde(rename = "SignedSuffix")]
    pub(crate) signed_suffix: String,
    #[serde(rename = "MailboxIDs")]
    pub(crate) mailbox_ids: Vec<i64>,
    #[serde(rename = "AliasName", default, skip_serializing_if = "Option::is_none")]
    pub(crate) alias_name: Option<String>,
    #[serde(rename = "Item")]
    pub(crate) item: CreateItemRequest,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AliasMailboxesRequest {
    #[serde(rename = "MailboxIDs")]
    pub(crate) mailbox_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AliasStatusRequest {
    #[serde(rename = "Enable")]
    pub(crate) enable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AliasNameRequest {
    #[serde(rename = "Name")]
    pub(crate) name: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AliasNoteRequest {
    #[serde(rename = "Note")]
    pub(crate) note: String,
}
