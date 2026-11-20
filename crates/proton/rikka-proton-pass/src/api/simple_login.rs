use serde::{Deserialize, Serialize};

use crate::api::CreateItemRequest;
use crate::identifier::{PendingAliasId, ShareId};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AliasSyncStatus {
    #[serde(rename = "Enabled", default)]
    enabled: bool,
    #[serde(rename = "PendingAliasCount", default)]
    pending_alias_count: i64,
}

impl AliasSyncStatus {
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn pending_alias_count(&self) -> i64 {
        self.pending_alias_count
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAlias {
    #[serde(rename = "PendingAliasID")]
    id: PendingAliasId,
    #[serde(rename = "AliasEmail", default)]
    email: String,
    #[serde(rename = "AliasNote", default)]
    note: String,
}

impl PendingAlias {
    #[must_use]
    pub fn id(&self) -> &PendingAliasId {
        &self.id
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn note(&self) -> &str {
        &self.note
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasDomain {
    #[serde(rename = "Domain", default)]
    domain: String,
    #[serde(rename = "IsCustom", default)]
    custom: bool,
    #[serde(rename = "IsPremium", default)]
    premium: bool,
    #[serde(rename = "MXVerified", default)]
    mx_verified: bool,
    #[serde(rename = "IsDefault", default)]
    default: bool,
}

impl AliasDomain {
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
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
    pub fn is_mx_verified(&self) -> bool {
        self.mx_verified
    }

    #[must_use]
    pub fn is_default(&self) -> bool {
        self.default
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AliasSettings {
    #[serde(rename = "DefaultAliasDomain", default)]
    default_alias_domain: Option<String>,
    #[serde(rename = "DefaultMailboxID", default)]
    default_mailbox_id: i64,
}

impl AliasSettings {
    #[must_use]
    pub fn default_alias_domain(&self) -> Option<&str> {
        self.default_alias_domain.as_deref()
    }

    #[must_use]
    pub fn default_mailbox_id(&self) -> i64 {
        self.default_mailbox_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMailbox {
    #[serde(rename = "MailboxID")]
    id: i64,
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "PendingEmail", default)]
    pending_email: Option<String>,
    #[serde(rename = "Verified", default)]
    verified: bool,
    #[serde(rename = "IsDefault", default)]
    default: bool,
    #[serde(rename = "AliasCount", default)]
    alias_count: i64,
    #[serde(rename = "CanBeEdited", default)]
    editable: Option<bool>,
}

impl UserMailbox {
    #[must_use]
    pub fn id(&self) -> i64 {
        self.id
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn pending_email(&self) -> Option<&str> {
        self.pending_email.as_deref()
    }

    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.verified
    }

    #[must_use]
    pub fn is_default(&self) -> bool {
        self.default
    }

    #[must_use]
    pub fn alias_count(&self) -> i64 {
        self.alias_count
    }

    #[must_use]
    pub fn can_be_edited(&self) -> bool {
        self.editable.unwrap_or(true)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EnableSyncRequest<'a> {
    #[serde(rename = "DefaultShareID", skip_serializing_if = "Option::is_none")]
    pub(crate) default_share: Option<&'a ShareId>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PendingAliasPage {
    #[serde(rename = "Aliases", default)]
    pub(crate) aliases: Vec<PendingAlias>,
    #[serde(rename = "LastToken", default)]
    pub(crate) last_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreatePendingAliasesRequest {
    #[serde(rename = "Items")]
    pub(crate) items: Vec<CreatePendingAlias>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreatePendingAlias {
    #[serde(rename = "PendingAliasID")]
    pub(crate) id: PendingAliasId,
    #[serde(rename = "Item")]
    pub(crate) item: CreateItemRequest,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DefaultDomainRequest<'a> {
    #[serde(rename = "DefaultAliasDomain")]
    pub(crate) domain: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DefaultMailboxRequest {
    #[serde(rename = "DefaultMailboxID")]
    pub(crate) mailbox: i64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MailboxEmailRequest<'a> {
    #[serde(rename = "Email")]
    pub(crate) email: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct VerificationCodeRequest<'a> {
    #[serde(rename = "Code")]
    pub(crate) code: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DeleteMailboxRequest {
    #[serde(rename = "TransferMailboxID")]
    pub(crate) transfer_to: Option<i64>,
}
