use jiff::Timestamp;
use rikka_proton_account::{AddressId, AddressKey};
use serde::{Deserialize, Serialize};

use crate::api::{InviteKey, InviteVaultData, ShareTarget};
use crate::identifier::{GroupId, InviteId, InviteToken};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupAddress {
    #[serde(rename = "ID")]
    id: AddressId,
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "Keys", default)]
    keys: Vec<AddressKey>,
}

impl GroupAddress {
    #[must_use]
    pub fn id(&self) -> &AddressId {
        &self.id
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn keys(&self) -> &[AddressKey] {
        &self.keys
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    #[serde(rename = "ID")]
    id: GroupId,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Address", default)]
    address: Option<GroupAddress>,
    #[serde(rename = "Permissions", default)]
    permissions: i64,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(rename = "Flags", default)]
    flags: i64,
    #[serde(rename = "GroupVisibility", default)]
    group_visibility: i64,
    #[serde(rename = "MemberVisibility", default)]
    member_visibility: i64,
    #[serde(rename = "Description", default)]
    description: Option<String>,
}

impl Group {
    #[must_use]
    pub fn id(&self) -> &GroupId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn address(&self) -> Option<&GroupAddress> {
        self.address.as_ref()
    }

    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.address.as_ref().map(GroupAddress::email)
    }

    #[must_use]
    pub fn permissions(&self) -> i64 {
        self.permissions
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn flags(&self) -> i64 {
        self.flags
    }

    #[must_use]
    pub fn group_visibility(&self) -> i64 {
        self.group_visibility
    }

    #[must_use]
    pub fn member_visibility(&self) -> i64 {
        self.member_visibility
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Type", default)]
    kind: i64,
    #[serde(rename = "State", default)]
    state: i64,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(rename = "GroupID")]
    group_id: GroupId,
    #[serde(rename = "AddressID", default)]
    address_id: Option<AddressId>,
    #[serde(rename = "Email", default)]
    email: Option<String>,
}

impl GroupMember {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn kind(&self) -> i64 {
        self.kind
    }

    #[must_use]
    pub fn state(&self) -> i64 {
        self.state
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn group_id(&self) -> &GroupId {
        &self.group_id
    }

    #[must_use]
    pub fn address_id(&self) -> Option<&AddressId> {
        self.address_id.as_ref()
    }

    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInviteResponse {
    #[serde(rename = "InviteID")]
    id: InviteId,
    #[serde(rename = "InviterUserID", default)]
    inviter_user_id: String,
    #[serde(rename = "InviterEmail", default)]
    inviter_email: String,
    #[serde(rename = "InvitedGroupID")]
    group_id: GroupId,
    #[serde(rename = "InvitedEmail", default)]
    invited_email: String,
    #[serde(rename = "TargetType", default)]
    target: ShareTarget,
    #[serde(rename = "TargetID", default)]
    target_id: String,
    #[serde(rename = "RemindersSent", default)]
    reminders_sent: i64,
    #[serde(rename = "InviteToken")]
    token: InviteToken,
    #[serde(rename = "InvitedAddressID", default)]
    invited_address_id: AddressId,
    #[serde(rename = "Keys", default)]
    keys: Vec<InviteKey>,
    #[serde(rename = "VaultData", default, skip_serializing_if = "Option::is_none")]
    vault_data: Option<InviteVaultData>,
    #[serde(rename = "IsGroupOwner", default)]
    group_owner: bool,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
}

impl GroupInviteResponse {
    #[must_use]
    pub fn id(&self) -> &InviteId {
        &self.id
    }

    #[must_use]
    pub fn inviter_user_id(&self) -> &str {
        &self.inviter_user_id
    }

    #[must_use]
    pub fn inviter_email(&self) -> &str {
        &self.inviter_email
    }

    #[must_use]
    pub fn group_id(&self) -> &GroupId {
        &self.group_id
    }

    #[must_use]
    pub fn invited_email(&self) -> &str {
        &self.invited_email
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
    pub fn reminders_sent(&self) -> i64 {
        self.reminders_sent
    }

    #[must_use]
    pub fn token(&self) -> &InviteToken {
        &self.token
    }

    #[must_use]
    pub fn invited_address_id(&self) -> &AddressId {
        &self.invited_address_id
    }

    #[must_use]
    pub fn keys(&self) -> &[InviteKey] {
        &self.keys
    }

    #[must_use]
    pub fn vault_data(&self) -> Option<&InviteVaultData> {
        self.vault_data.as_ref()
    }

    #[must_use]
    pub fn is_group_owner(&self) -> bool {
        self.group_owner
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GroupMembersEnvelope {
    #[serde(rename = "Members", default)]
    pub(crate) members: Vec<GroupMember>,
    #[serde(rename = "Total", default)]
    pub(crate) total: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GroupInvitePage {
    #[serde(rename = "Invites", default)]
    pub(crate) invites: Vec<GroupInviteResponse>,
    #[serde(rename = "LastID", default)]
    pub(crate) last_id: Option<String>,
}
