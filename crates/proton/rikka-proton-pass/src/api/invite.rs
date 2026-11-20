use jiff::Timestamp;
use rikka_proton_account::AddressId;
use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

use crate::api::ShareTarget;
use crate::identifier::{InviteId, InviteToken, ItemId, NewUserInviteId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteKey {
    #[serde(rename = "KeyRotation")]
    rotation: i64,
    #[serde(rename = "Key")]
    key: String,
}

impl InviteKey {
    pub(crate) fn new(rotation: i64, key: String) -> InviteKey {
        InviteKey { rotation, key }
    }

    #[must_use]
    pub fn rotation(&self) -> i64 {
        self.rotation
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteVaultData {
    #[serde(rename = "Content", default)]
    content: String,
    #[serde(rename = "ContentKeyRotation", default)]
    content_key_rotation: i64,
    #[serde(rename = "ContentFormatVersion", default)]
    content_format_version: i32,
    #[serde(rename = "MemberCount", default)]
    member_count: i64,
    #[serde(rename = "ItemCount", default)]
    item_count: i64,
}

impl InviteVaultData {
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub fn content_key_rotation(&self) -> i64 {
        self.content_key_rotation
    }

    #[must_use]
    pub fn content_format_version(&self) -> i32 {
        self.content_format_version
    }

    #[must_use]
    pub fn member_count(&self) -> i64 {
        self.member_count
    }

    #[must_use]
    pub fn item_count(&self) -> i64 {
        self.item_count
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInviteResponse {
    #[serde(rename = "InviteToken")]
    token: InviteToken,
    #[serde(rename = "RemindersSent", default)]
    reminders_sent: i64,
    #[serde(rename = "TargetType", default)]
    target: ShareTarget,
    #[serde(rename = "TargetID", default)]
    target_id: String,
    #[serde(rename = "InviterEmail", default)]
    inviter_email: String,
    #[serde(rename = "InvitedEmail", default)]
    invited_email: String,
    #[serde(rename = "InvitedAddressID", default)]
    invited_address_id: AddressId,
    #[serde(rename = "Keys", default)]
    keys: Vec<InviteKey>,
    #[serde(rename = "VaultData", default, skip_serializing_if = "Option::is_none")]
    vault_data: Option<InviteVaultData>,
    #[serde(rename = "FromNewUser", default)]
    from_new_user: bool,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
}

impl UserInviteResponse {
    #[must_use]
    pub fn token(&self) -> &InviteToken {
        &self.token
    }

    #[must_use]
    pub fn reminders_sent(&self) -> i64 {
        self.reminders_sent
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
    pub fn inviter_email(&self) -> &str {
        &self.inviter_email
    }

    #[must_use]
    pub fn invited_email(&self) -> &str {
        &self.invited_email
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
    pub fn is_from_new_user(&self) -> bool {
        self.from_new_user
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareInvite {
    #[serde(rename = "InviteID")]
    id: InviteId,
    #[serde(rename = "InvitedEmail", default)]
    invited_email: String,
    #[serde(rename = "InviterEmail", default)]
    inviter_email: String,
    #[serde(rename = "ShareRoleID", default)]
    role_id: String,
    #[serde(rename = "TargetType", default)]
    target: ShareTarget,
    #[serde(rename = "TargetID", default)]
    target_id: String,
    #[serde(rename = "RemindersSent", default)]
    reminders_sent: i64,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(
        rename = "ModifyTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    modify_time: Timestamp,
}

impl ShareInvite {
    #[must_use]
    pub fn id(&self) -> &InviteId {
        &self.id
    }

    #[must_use]
    pub fn invited_email(&self) -> &str {
        &self.invited_email
    }

    #[must_use]
    pub fn inviter_email(&self) -> &str {
        &self.inviter_email
    }

    #[must_use]
    pub fn role_id(&self) -> &str {
        &self.role_id
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
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn modify_time(&self) -> Timestamp {
        self.modify_time
    }
}

wire_enum! {
    pub enum NewUserInviteState: default WaitingForAccount {
        WaitingForAccount = 1,
        Ready = 2,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUserInvite {
    #[serde(rename = "NewUserInviteID")]
    id: NewUserInviteId,
    #[serde(rename = "State", default)]
    state: NewUserInviteState,
    #[serde(rename = "TargetType", default)]
    target: ShareTarget,
    #[serde(rename = "TargetID", default)]
    target_id: String,
    #[serde(rename = "ShareRoleID", default)]
    role_id: String,
    #[serde(rename = "InvitedEmail", default)]
    invited_email: String,
    #[serde(rename = "InviterEmail", default)]
    inviter_email: String,
    #[serde(rename = "Signature", default)]
    signature: String,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(
        rename = "ModifyTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    modify_time: Timestamp,
}

impl NewUserInvite {
    #[must_use]
    pub fn id(&self) -> &NewUserInviteId {
        &self.id
    }

    #[must_use]
    pub fn state(&self) -> NewUserInviteState {
        self.state
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.state == NewUserInviteState::Ready
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
    pub fn role_id(&self) -> &str {
        &self.role_id
    }

    #[must_use]
    pub fn invited_email(&self) -> &str {
        &self.invited_email
    }

    #[must_use]
    pub fn inviter_email(&self) -> &str {
        &self.inviter_email
    }

    #[must_use]
    pub fn signature(&self) -> &str {
        &self.signature
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn modify_time(&self) -> Timestamp {
        self.modify_time
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ShareInvites {
    #[serde(rename = "Invites", default)]
    invites: Vec<ShareInvite>,
    #[serde(rename = "NewUserInvites", default)]
    new_user_invites: Vec<NewUserInvite>,
}

impl ShareInvites {
    #[must_use]
    pub fn invites(&self) -> &[ShareInvite] {
        &self.invites
    }

    #[must_use]
    pub fn new_user_invites(&self) -> &[NewUserInvite] {
        &self.new_user_invites
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InviteSuggestion {
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "AddressID", default)]
    address_id: Option<AddressId>,
    #[serde(rename = "IsGroup", default)]
    group: bool,
}

impl InviteSuggestion {
    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn address_id(&self) -> Option<&AddressId> {
        self.address_id.as_ref()
    }

    #[must_use]
    pub fn is_group(&self) -> bool {
        self.group
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrganizationRecommendation {
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "AlreadyHasAccess", default)]
    has_access: bool,
}

impl OrganizationRecommendation {
    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn has_access(&self) -> bool {
        self.has_access
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrganizationRecommendations {
    #[serde(rename = "GroupDisplayName", default)]
    group_display_name: Option<String>,
    #[serde(rename = "NextToken", default)]
    next_token: Option<String>,
    #[serde(rename = "Entries", default)]
    entries: Vec<OrganizationRecommendation>,
}

impl OrganizationRecommendations {
    #[must_use]
    pub fn group_display_name(&self) -> Option<&str> {
        self.group_display_name.as_deref()
    }

    #[must_use]
    pub fn next_token(&self) -> Option<&str> {
        self.next_token.as_deref()
    }

    #[must_use]
    pub fn entries(&self) -> &[OrganizationRecommendation] {
        &self.entries
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateInviteRequest {
    #[serde(rename = "Keys")]
    pub(crate) keys: Vec<InviteKey>,
    #[serde(rename = "Email")]
    pub(crate) email: String,
    #[serde(rename = "TargetType")]
    pub(crate) target: ShareTarget,
    #[serde(rename = "ShareRoleID")]
    pub(crate) role_id: &'static str,
    #[serde(rename = "ItemID", skip_serializing_if = "Option::is_none")]
    pub(crate) item_id: Option<ItemId>,
    #[serde(
        rename = "ExpirationTime",
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    pub(crate) expiration_time: Option<Timestamp>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateNewUserInviteRequest {
    #[serde(rename = "Email")]
    pub(crate) email: String,
    #[serde(rename = "TargetType")]
    pub(crate) target: ShareTarget,
    #[serde(rename = "Signature")]
    pub(crate) signature: String,
    #[serde(rename = "ShareRoleID")]
    pub(crate) role_id: &'static str,
    #[serde(rename = "ItemID", skip_serializing_if = "Option::is_none")]
    pub(crate) item_id: Option<ItemId>,
    #[serde(
        rename = "ExpirationTime",
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    pub(crate) expiration_time: Option<Timestamp>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateInvitesRequest {
    #[serde(rename = "Invites")]
    pub(crate) invites: Vec<CreateInviteRequest>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateNewUserInvitesRequest {
    #[serde(rename = "NewUserInvites")]
    pub(crate) invites: Vec<CreateNewUserInviteRequest>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct InviteKeysRequest {
    #[serde(rename = "Keys")]
    pub(crate) keys: Vec<InviteKey>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CheckAddressesRequest<'a> {
    #[serde(rename = "Emails")]
    pub(crate) emails: &'a [String],
}
