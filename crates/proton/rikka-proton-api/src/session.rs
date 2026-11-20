use std::fmt;

use serde::{Deserialize, Serialize};

use crate::mask::Masked;

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    #[serde(rename = "UID")]
    uid: String,
    #[serde(rename = "AccessToken")]
    access_token: String,
    #[serde(rename = "RefreshToken")]
    refresh_token: String,
    #[serde(
        rename = "Scopes",
        default,
        deserialize_with = "crate::scopes::deserialize"
    )]
    scopes: Vec<String>,
    #[serde(rename = "UserID", default, skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
}

impl Session {
    pub fn new(
        uid: impl Into<String>,
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
    ) -> Session {
        Session {
            uid: uid.into(),
            access_token: access_token.into(),
            refresh_token: refresh_token.into(),
            scopes: Vec::new(),
            user_id: None,
        }
    }

    #[must_use]
    pub fn with_scopes(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Session {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Session {
        self.user_id = Some(user_id.into());
        self
    }

    #[must_use]
    pub fn uid(&self) -> &str {
        &self.uid
    }

    #[must_use]
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    #[must_use]
    pub fn refresh_token(&self) -> &str {
        &self.refresh_token
    }

    #[must_use]
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }

    #[must_use]
    pub fn user_id(&self) -> Option<&str> {
        self.user_id.as_deref()
    }

    #[must_use]
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|known| known == scope)
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.uid.trim().is_empty()
            && !self.access_token.trim().is_empty()
            && !self.refresh_token.trim().is_empty()
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("uid", &Masked(&self.uid))
            .field("access_token", &Masked(&self.access_token))
            .field("refresh_token", &Masked(&self.refresh_token))
            .field("scopes", &self.scopes)
            .field("user_id", &self.user_id)
            .finish()
    }
}
