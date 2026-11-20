use jiff::Timestamp;
use rikka_proton_api::wire_enum;
use rikka_proton_pgp::Secret;
use serde::Deserialize;

use crate::identifier::{LinkId, ShareId, ShareUrlId};
use crate::role::Role;

wire_enum! {
    pub enum ShareUrlFlags: default GeneratedPassword {
        Legacy = 0,
        CustomPassword = 1,
        GeneratedPassword = 2,
        GeneratedAndCustomPassword = 3,
    }
}

#[derive(Debug, Clone)]
pub struct ShareUrl {
    pub(super) wire: ShareUrlWire,
    pub(super) password: Secret,
    pub(super) custom_password: Option<Secret>,
}

impl ShareUrl {
    #[must_use]
    pub fn id(&self) -> &ShareUrlId {
        &self.wire.id
    }

    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.wire.share_id
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.wire.token
    }

    #[must_use]
    pub fn public_url(&self) -> &str {
        &self.wire.public_url
    }

    #[must_use]
    pub fn url(&self) -> String {
        format!("{}#{}", self.wire.public_url, self.password.expose())
    }

    #[must_use]
    pub fn password(&self) -> &Secret {
        &self.password
    }

    #[must_use]
    pub fn custom_password(&self) -> Option<&Secret> {
        self.custom_password.as_ref()
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.wire.role
    }

    #[must_use]
    pub fn flags(&self) -> ShareUrlFlags {
        self.wire.flags
    }

    #[must_use]
    pub fn creator(&self) -> &str {
        &self.wire.creator
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.wire.created_at
    }

    #[must_use]
    pub fn expires_at(&self) -> Option<Timestamp> {
        self.wire.expires_at
    }

    #[must_use]
    pub fn last_accessed_at(&self) -> Option<Timestamp> {
        self.wire.last_accessed_at
    }

    #[must_use]
    pub fn accesses(&self) -> u64 {
        self.wire.accesses
    }

    #[must_use]
    pub fn max_accesses(&self) -> u64 {
        self.wire.max_accesses
    }
}

#[derive(Debug, Clone)]
pub struct ShareUrlContext {
    pub(super) context_share_id: ShareId,
    pub(super) urls: Vec<ShareUrl>,
    pub(super) link_ids: Vec<LinkId>,
}

impl ShareUrlContext {
    #[must_use]
    pub fn context_share_id(&self) -> &ShareId {
        &self.context_share_id
    }

    #[must_use]
    pub fn urls(&self) -> &[ShareUrl] {
        &self.urls
    }

    #[must_use]
    pub fn link_ids(&self) -> &[LinkId] {
        &self.link_ids
    }
}

#[must_use]
#[derive(Debug, Clone, Default)]
pub struct ShareUrlSettings {
    pub(super) editable: bool,
    pub(super) custom_password: Option<Secret>,
    pub(super) expires_at: Option<Timestamp>,
}

impl ShareUrlSettings {
    pub fn new() -> ShareUrlSettings {
        ShareUrlSettings::default()
    }

    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    pub fn custom_password(mut self, password: impl Into<String>) -> Self {
        let password = Secret::new(password);
        self.custom_password = (!password.is_empty()).then_some(password);
        self
    }

    pub fn expires_at(mut self, at: Timestamp) -> Self {
        self.expires_at = Some(at);
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ShareUrlWire {
    #[serde(rename = "ShareURLID")]
    pub(super) id: ShareUrlId,
    #[serde(rename = "ShareID")]
    pub(super) share_id: ShareId,
    #[serde(rename = "Token", default)]
    pub(super) token: String,
    #[serde(rename = "PublicUrl", default)]
    pub(super) public_url: String,
    #[serde(rename = "Permissions", default)]
    pub(super) role: Role,
    #[serde(rename = "Flags", default)]
    pub(super) flags: ShareUrlFlags,
    #[serde(rename = "CreatorEmail", default)]
    pub(super) creator: String,
    #[serde(rename = "Password")]
    pub(super) password: String,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    pub(super) created_at: Option<Timestamp>,
    #[serde(
        rename = "ExpirationTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    pub(super) expires_at: Option<Timestamp>,
    #[serde(
        rename = "LastAccessTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    pub(super) last_accessed_at: Option<Timestamp>,
    #[serde(rename = "NumAccesses", default)]
    pub(super) accesses: u64,
    #[serde(rename = "MaxAccesses", default)]
    pub(super) max_accesses: u64,
}
