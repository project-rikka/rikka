use std::fmt;

use rikka_proton_api::Session;
use serde::{Deserialize, Serialize};

use crate::identifier::UserId;
use crate::password_mode::PasswordMode;
use crate::second_factor::SecondFactorInfo;

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    #[serde(rename = "UID")]
    uid: String,
    #[serde(rename = "UserID", default, skip_serializing_if = "Option::is_none")]
    user_id: Option<UserId>,
    #[serde(rename = "AccessToken")]
    access_token: String,
    #[serde(rename = "RefreshToken")]
    refresh_token: String,
    #[serde(rename = "TokenType", default)]
    token_type: String,
    #[serde(
        rename = "Scopes",
        default,
        deserialize_with = "rikka_proton_api::scopes::deserialize"
    )]
    scopes: Vec<String>,
    #[serde(
        rename = "ServerProof",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    server_proof: Option<String>,
    #[serde(rename = "PasswordMode", default)]
    password_mode: PasswordMode,
    #[serde(rename = "2FA", default, skip_serializing_if = "Option::is_none")]
    second_factor: Option<SecondFactorInfo>,
    #[serde(rename = "EventID", default, skip_serializing_if = "Option::is_none")]
    event_id: Option<String>,
    #[serde(rename = "LocalID", default, skip_serializing_if = "Option::is_none")]
    local_id: Option<i32>,
    #[serde(
        rename = "TemporaryPassword",
        default,
        with = "rikka_proton_api::flag::optional",
        skip_serializing_if = "Option::is_none"
    )]
    temporary_password: Option<bool>,
}

impl SessionInfo {
    #[must_use]
    pub fn uid(&self) -> &str {
        &self.uid
    }

    #[must_use]
    pub fn user_id(&self) -> Option<&UserId> {
        self.user_id.as_ref()
    }

    #[must_use]
    pub fn token_type(&self) -> &str {
        &self.token_type
    }

    #[must_use]
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }

    #[must_use]
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|known| known == scope)
    }

    #[must_use]
    pub fn server_proof(&self) -> Option<&str> {
        self.server_proof.as_deref()
    }

    #[must_use]
    pub fn password_mode(&self) -> PasswordMode {
        self.password_mode
    }

    #[must_use]
    pub fn second_factor(&self) -> Option<&SecondFactorInfo> {
        self.second_factor.as_ref()
    }

    #[must_use]
    pub fn needs_second_factor(&self) -> bool {
        self.second_factor
            .as_ref()
            .is_some_and(SecondFactorInfo::is_enabled)
    }

    #[must_use]
    pub fn event_id(&self) -> Option<&str> {
        self.event_id.as_deref()
    }

    #[must_use]
    pub fn local_id(&self) -> Option<i32> {
        self.local_id
    }

    #[must_use]
    pub fn has_temporary_password(&self) -> bool {
        self.temporary_password.unwrap_or_default()
    }

    #[must_use]
    pub fn session(&self) -> Session {
        let session = Session::new(&self.uid, &self.access_token, &self.refresh_token)
            .with_scopes(self.scopes.clone());

        match &self.user_id {
            Some(user_id) => session.with_user_id(user_id.as_str()),
            None => session,
        }
    }

    pub(crate) fn set_scopes(&mut self, scopes: Vec<String>) {
        self.scopes = scopes;
    }

    pub(crate) fn clear_second_factor(&mut self) {
        self.second_factor = None;
    }
}

impl fmt::Debug for SessionInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionInfo")
            .field("uid", &self.uid)
            .field("user_id", &self.user_id)
            .field("scopes", &self.scopes)
            .field("password_mode", &self.password_mode)
            .field("second_factor", &self.second_factor)
            .field("event_id", &self.event_id)
            .finish_non_exhaustive()
    }
}
