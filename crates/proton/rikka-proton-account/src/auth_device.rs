use jiff::Timestamp;
use std::fmt;

use rikka_proton_api::{ResponseCode, identifier, wire_enum};
use serde::{Deserialize, Serialize};

use crate::identifier::{AddressId, UserId};

identifier!(AuthDeviceId);

wire_enum! {
    pub enum AuthDeviceState: default Inactive {
        Inactive = 0,
        Active = 1,
        PendingActivation = 2,
        PendingAdminActivation = 3,
        Rejected = 4,
        NoSession = 5,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthDevice {
    #[serde(rename = "ID")]
    id: AuthDeviceId,
    #[serde(rename = "MemberID", default, skip_serializing_if = "Option::is_none")]
    member_id: Option<UserId>,
    #[serde(rename = "State", default)]
    state: AuthDeviceState,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "LocalizedClientName", default)]
    client_name: String,
    #[serde(rename = "Platform", default, skip_serializing_if = "Option::is_none")]
    platform: Option<String>,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    created_at: Timestamp,
    #[serde(
        rename = "ActivateTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    activated_at: Option<Timestamp>,
    #[serde(
        rename = "RejectTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    rejected_at: Option<Timestamp>,
    #[serde(
        rename = "LastActivityTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    last_activity_at: Timestamp,
    #[serde(
        rename = "ActivationToken",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    activation_token: Option<String>,
    #[serde(
        rename = "ActivationAddressID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    activation_address: Option<AddressId>,
    #[serde(
        rename = "ConfirmationCodeVersion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    confirmation_code_version: Option<u8>,
}

impl AuthDevice {
    #[must_use]
    pub fn id(&self) -> &AuthDeviceId {
        &self.id
    }

    #[must_use]
    pub fn member_id(&self) -> Option<&UserId> {
        self.member_id.as_ref()
    }

    #[must_use]
    pub fn state(&self) -> AuthDeviceState {
        self.state
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state == AuthDeviceState::Active
    }

    #[must_use]
    pub fn is_pending_activation(&self) -> bool {
        self.state == AuthDeviceState::PendingActivation
    }

    #[must_use]
    pub fn is_pending_admin_activation(&self) -> bool {
        self.state == AuthDeviceState::PendingAdminActivation
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    #[must_use]
    pub fn platform(&self) -> Option<&str> {
        self.platform.as_deref()
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    #[must_use]
    pub fn activated_at(&self) -> Option<Timestamp> {
        self.activated_at
    }

    #[must_use]
    pub fn rejected_at(&self) -> Option<Timestamp> {
        self.rejected_at
            .filter(|time| *time > Timestamp::UNIX_EPOCH)
    }

    #[must_use]
    pub fn last_activity_at(&self) -> Timestamp {
        self.last_activity_at
    }

    #[must_use]
    pub fn activation_token(&self) -> Option<&str> {
        self.activation_token.as_deref()
    }

    #[must_use]
    pub fn activation_address(&self) -> Option<&AddressId> {
        self.activation_address.as_ref()
    }

    #[must_use]
    pub fn confirmation_code_version(&self) -> u8 {
        self.confirmation_code_version.unwrap_or(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AuthDeviceError {
    NotFound,
    NotActive,
    TokenInvalid,
    Rejected,
    SessionAlreadyAssociated,
}

impl AuthDeviceError {
    pub(crate) fn from_api_error(error: &rikka_proton_api::Error) -> Option<AuthDeviceError> {
        match error.code()? {
            ResponseCode::AuthDeviceNotFound => Some(AuthDeviceError::NotFound),
            ResponseCode::AuthDeviceNotActive => Some(AuthDeviceError::NotActive),
            ResponseCode::AuthDeviceTokenInvalid => Some(AuthDeviceError::TokenInvalid),
            ResponseCode::AuthDeviceRejected => Some(AuthDeviceError::Rejected),
            ResponseCode::NotAllowed => Some(AuthDeviceError::SessionAlreadyAssociated),
            _ => None,
        }
    }

    #[must_use]
    pub fn invalidates_device(self) -> bool {
        matches!(
            self,
            AuthDeviceError::NotFound | AuthDeviceError::TokenInvalid | AuthDeviceError::Rejected
        )
    }
}

impl fmt::Display for AuthDeviceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            AuthDeviceError::NotFound => "device not found",
            AuthDeviceError::NotActive => "device pending activation",
            AuthDeviceError::TokenInvalid => "invalid device token",
            AuthDeviceError::Rejected => "device rejected",
            AuthDeviceError::SessionAlreadyAssociated => "session already associated with a device",
        })
    }
}
