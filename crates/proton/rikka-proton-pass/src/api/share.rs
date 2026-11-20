use jiff::Timestamp;
use rikka_proton_account::AddressId;
use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

use crate::identifier::{GroupId, ShareId, VaultId};

wire_enum! {
    pub enum ShareTarget: default Vault {
        Vault = 1,
        Item = 2,
        Folder = 3,
    }
}

impl ShareTarget {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            ShareTarget::Vault => "vault",
            ShareTarget::Item => "item",
            ShareTarget::Folder => "folder",
            ShareTarget::Other(_) => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareRole {
    Owner,
    Manager,
    Editor,
    Viewer,
    Other(String),
}

impl ShareRole {
    pub(crate) fn from_role_id(role_id: &str, is_owner: bool) -> ShareRole {
        if is_owner {
            return ShareRole::Owner;
        }

        match role_id {
            "1" => ShareRole::Manager,
            "2" => ShareRole::Editor,
            "3" => ShareRole::Viewer,
            other => ShareRole::Other(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemberRole {
    Manager,
    Editor,
    Viewer,
}

impl MemberRole {
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            MemberRole::Manager => "1",
            MemberRole::Editor => "2",
            MemberRole::Viewer => "3",
        }
    }
}

impl From<MemberRole> for ShareRole {
    fn from(role: MemberRole) -> ShareRole {
        match role {
            MemberRole::Manager => ShareRole::Manager,
            MemberRole::Editor => ShareRole::Editor,
            MemberRole::Viewer => ShareRole::Viewer,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareResponse {
    #[serde(rename = "ShareID")]
    share_id: ShareId,
    #[serde(rename = "VaultID", default)]
    vault_id: VaultId,
    #[serde(rename = "AddressID", default)]
    address_id: AddressId,
    #[serde(rename = "GroupID", default, skip_serializing_if = "Option::is_none")]
    group_id: Option<String>,
    #[serde(rename = "TargetType", default)]
    target: ShareTarget,
    #[serde(rename = "TargetID", default)]
    target_id: String,
    #[serde(rename = "Permission", default)]
    permission: i64,
    #[serde(rename = "Content", default, skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(
        rename = "ContentKeyRotation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    content_key_rotation: Option<i64>,
    #[serde(
        rename = "ContentFormatVersion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    content_format_version: Option<i32>,
    #[serde(rename = "ShareRoleID", default)]
    role_id: String,
    #[serde(rename = "TargetMaxMembers", default)]
    target_max_members: i64,
    #[serde(rename = "TargetMembers", default)]
    target_members: i64,
    #[serde(rename = "PendingInvites", default)]
    pending_invites: i64,
    #[serde(rename = "NewUserInvitesReady", default)]
    new_user_invites_ready: i64,
    #[serde(rename = "Owner", default)]
    owner: bool,
    #[serde(rename = "Shared", default)]
    shared: bool,
    #[serde(rename = "CanAutoFill", default)]
    can_autofill: bool,
    #[serde(
        rename = "ExpireTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    expire_time: Option<Timestamp>,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(rename = "Flags", default)]
    flags: i64,
}

impl ShareResponse {
    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn vault_id(&self) -> &VaultId {
        &self.vault_id
    }

    #[must_use]
    pub fn address_id(&self) -> &AddressId {
        &self.address_id
    }

    #[must_use]
    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }

    #[must_use]
    pub fn group(&self) -> Option<GroupId> {
        self.group_id
            .as_deref()
            .filter(|group| !group.is_empty())
            .map(GroupId::from)
    }
    #[must_use]
    pub fn target(&self) -> ShareTarget {
        self.target
    }

    #[must_use]
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    #[must_use]
    pub fn permission(&self) -> i64 {
        self.permission
    }

    #[must_use]
    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    #[must_use]
    pub fn content_key_rotation(&self) -> Option<i64> {
        self.content_key_rotation
    }

    #[must_use]
    pub fn content_format_version(&self) -> Option<i32> {
        self.content_format_version
    }

    #[must_use]
    pub fn role(&self) -> ShareRole {
        ShareRole::from_role_id(&self.role_id, self.owner)
    }

    #[must_use]
    pub fn role_id(&self) -> &str {
        &self.role_id
    }

    #[must_use]
    pub fn target_max_members(&self) -> i64 {
        self.target_max_members
    }

    #[must_use]
    pub fn target_members(&self) -> i64 {
        self.target_members
    }

    #[must_use]
    pub fn pending_invites(&self) -> i64 {
        self.pending_invites
    }

    #[must_use]
    pub fn new_user_invites_ready(&self) -> i64 {
        self.new_user_invites_ready
    }

    #[must_use]
    pub fn is_owner(&self) -> bool {
        self.owner
    }

    #[must_use]
    pub fn is_shared(&self) -> bool {
        self.shared
    }

    #[must_use]
    pub fn can_autofill(&self) -> bool {
        self.can_autofill
    }

    #[must_use]
    pub fn expire_time(&self) -> Option<Timestamp> {
        self.expire_time
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn flags(&self) -> i64 {
        self.flags
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareKeyResponse {
    #[serde(rename = "KeyRotation")]
    rotation: i64,
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "UserKeyID", default, skip_serializing_if = "Option::is_none")]
    user_key_id: Option<String>,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
}

impl ShareKeyResponse {
    #[must_use]
    pub fn rotation(&self) -> i64 {
        self.rotation
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn user_key_id(&self) -> Option<&str> {
        self.user_key_id.as_deref()
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }
}
