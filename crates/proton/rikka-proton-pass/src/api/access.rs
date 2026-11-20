use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::identifier::ShareId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassPlan {
    #[serde(rename = "Type", default)]
    kind: String,
    #[serde(rename = "InternalName", default)]
    internal_name: String,
    #[serde(rename = "DisplayName", default)]
    display_name: String,
    #[serde(
        rename = "VaultLimit",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    vault_limit: Option<i64>,
    #[serde(
        rename = "AliasLimit",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    alias_limit: Option<i64>,
    #[serde(rename = "TotpLimit", default, skip_serializing_if = "Option::is_none")]
    totp_limit: Option<i64>,
    #[serde(rename = "HideUpgrade", default)]
    hide_upgrade: bool,
    #[serde(
        rename = "TrialEnd",
        default,
        with = "rikka_proton_api::unix_seconds::optional",
        skip_serializing_if = "Option::is_none"
    )]
    trial_end: Option<Timestamp>,
    #[serde(rename = "ManageAlias", default)]
    manage_alias: bool,
    #[serde(rename = "StorageAllowed", default)]
    storage_allowed: bool,
    #[serde(rename = "StorageUsed", default)]
    storage_used: i64,
    #[serde(rename = "StorageQuota", default)]
    storage_quota: i64,
    #[serde(rename = "StorageMaxFileSize", default)]
    storage_max_file_size: i64,
    #[serde(rename = "FolderAllowed", default)]
    folder_allowed: bool,
    #[serde(
        rename = "FolderMaxCount",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    folder_max_count: Option<i64>,
    #[serde(
        rename = "FolderMaxChildren",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    folder_max_children: Option<i64>,
    #[serde(
        rename = "FolderMaxDepth",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    folder_max_depth: Option<i64>,
}

impl PassPlan {
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn internal_name(&self) -> &str {
        &self.internal_name
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn vault_limit(&self) -> Option<i64> {
        self.vault_limit
    }

    #[must_use]
    pub fn alias_limit(&self) -> Option<i64> {
        self.alias_limit
    }

    #[must_use]
    pub fn totp_limit(&self) -> Option<i64> {
        self.totp_limit
    }

    #[must_use]
    pub fn hides_upgrade(&self) -> bool {
        self.hide_upgrade
    }

    #[must_use]
    pub fn trial_end(&self) -> Option<Timestamp> {
        self.trial_end
    }

    #[must_use]
    pub fn manages_aliases(&self) -> bool {
        self.manage_alias
    }

    #[must_use]
    pub fn allows_storage(&self) -> bool {
        self.storage_allowed
    }

    #[must_use]
    pub fn storage_used(&self) -> i64 {
        self.storage_used
    }

    #[must_use]
    pub fn storage_quota(&self) -> i64 {
        self.storage_quota
    }

    #[must_use]
    pub fn storage_max_file_size(&self) -> i64 {
        self.storage_max_file_size
    }

    #[must_use]
    pub fn allows_folders(&self) -> bool {
        self.folder_allowed
    }

    #[must_use]
    pub fn folder_max_count(&self) -> Option<i64> {
        self.folder_max_count
    }

    #[must_use]
    pub fn folder_max_children(&self) -> Option<i64> {
        self.folder_max_children
    }

    #[must_use]
    pub fn folder_max_depth(&self) -> Option<i64> {
        self.folder_max_depth
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct MonitorState {
    #[serde(rename = "ProtonAddress", default)]
    proton_addresses: bool,
    #[serde(rename = "Aliases", default)]
    aliases: bool,
}

impl MonitorState {
    #[must_use]
    pub fn watches_proton_addresses(&self) -> bool {
        self.proton_addresses
    }

    #[must_use]
    pub fn watches_aliases(&self) -> bool {
        self.aliases
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserData {
    #[serde(
        rename = "DefaultShareID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    default_share: Option<ShareId>,
    #[serde(rename = "AliasSyncEnabled", default)]
    alias_sync: bool,
    #[serde(rename = "PendingAliasToSync", default)]
    pending_aliases: i64,
}

impl UserData {
    #[must_use]
    pub fn default_share(&self) -> Option<&ShareId> {
        self.default_share.as_ref()
    }

    #[must_use]
    pub fn syncs_aliases(&self) -> bool {
        self.alias_sync
    }

    #[must_use]
    pub fn pending_aliases(&self) -> i64 {
        self.pending_aliases
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccess {
    #[serde(rename = "Plan")]
    plan: PassPlan,
    #[serde(rename = "Monitor", default)]
    monitor: MonitorState,
    #[serde(rename = "PendingInvites", default)]
    pending_invites: i64,
    #[serde(rename = "WaitingNewUserInvites", default)]
    waiting_new_user_invites: i64,
    #[serde(
        rename = "MinVersionUpgrade",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    min_version_upgrade: Option<String>,
    #[serde(rename = "UserData", default)]
    user_data: UserData,
}

impl UserAccess {
    #[must_use]
    pub fn plan(&self) -> &PassPlan {
        &self.plan
    }

    #[must_use]
    pub fn monitor(&self) -> MonitorState {
        self.monitor
    }

    #[must_use]
    pub fn pending_invites(&self) -> i64 {
        self.pending_invites
    }

    #[must_use]
    pub fn waiting_new_user_invites(&self) -> i64 {
        self.waiting_new_user_invites
    }

    #[must_use]
    pub fn min_version_upgrade(&self) -> Option<&str> {
        self.min_version_upgrade.as_deref()
    }

    #[must_use]
    pub fn user_data(&self) -> &UserData {
        &self.user_data
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LockSessionRequest {
    #[serde(rename = "LockCode")]
    pub(crate) lock_code: String,
    #[serde(rename = "UnlockedSecs")]
    pub(crate) unlocked_seconds: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SessionCodeRequest {
    #[serde(rename = "LockCode")]
    pub(crate) lock_code: String,
}
