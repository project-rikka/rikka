use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::api::{ShareRole, ShareTarget};
use crate::identifier::ShareId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareMember {
    #[serde(rename = "ShareID")]
    share_id: ShareId,
    #[serde(rename = "UserName", default)]
    user_name: String,
    #[serde(rename = "UserEmail", default)]
    user_email: String,
    #[serde(rename = "Owner", default)]
    owner: Option<bool>,
    #[serde(rename = "TargetType", default)]
    target: ShareTarget,
    #[serde(rename = "TargetID", default)]
    target_id: String,
    #[serde(rename = "Permission", default)]
    permission: i64,
    #[serde(rename = "ShareRoleID", default)]
    role_id: Option<String>,
    #[serde(rename = "IsGroupShare", default)]
    group_share: bool,
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
}

impl ShareMember {
    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn user_name(&self) -> &str {
        &self.user_name
    }

    #[must_use]
    pub fn user_email(&self) -> &str {
        &self.user_email
    }

    #[must_use]
    pub fn is_owner(&self) -> bool {
        self.owner.unwrap_or_default()
    }

    #[must_use]
    pub fn role(&self) -> ShareRole {
        ShareRole::from_role_id(self.role_id.as_deref().unwrap_or_default(), self.is_owner())
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
    pub fn is_group_share(&self) -> bool {
        self.group_share
    }

    #[must_use]
    pub fn expire_time(&self) -> Option<Timestamp> {
        self.expire_time
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MembersEnvelope {
    #[serde(rename = "Shares", default)]
    pub(crate) shares: Vec<ShareMember>,
    #[serde(rename = "LastToken", default)]
    pub(crate) last_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UpdateMemberRequest {
    #[serde(rename = "ShareRoleID")]
    pub(crate) role_id: &'static str,
    #[serde(
        rename = "ExpireTime",
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    pub(crate) expire_time: Option<Timestamp>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TransferOwnershipRequest<'a> {
    #[serde(rename = "NewOwnerShareID")]
    pub(crate) new_owner: &'a ShareId,
}
