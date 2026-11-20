use rikka_proton_pgp::Secret;
use serde::Serialize;

use crate::account::Account;
use crate::error::Result;
use crate::reauthenticated_request::{ReauthenticatedRequest, Verb};
use crate::reauthentication::reauthenticated;
use crate::settings::UserSettings;
use crate::verifier::{self, Verifier};

const LOGIN_PASSWORD_PATH: &str = "core/v4/settings/password";

#[must_use]
#[derive(Debug)]
pub struct LoginPasswordChange {
    request: ReauthenticatedRequest,
    new_password: Secret,
}

reauthenticated!(LoginPasswordChange, request.reauthentication);

impl LoginPasswordChange {
    pub fn new_password(mut self, password: impl Into<String>) -> Self {
        self.new_password = Secret::new(password);
        self
    }

    pub fn keep_password_scope(mut self) -> Self {
        self.request.keep_password_scope();
        self
    }

    pub async fn execute(mut self) -> Result<UserSettings> {
        let auth = verifier::generate(self.request.account(), &self.new_password).await?;

        self.request.extend(&AuthRequest { auth })?;
        self.request.send_field_or_default("UserSettings").await
    }

    pub(crate) fn new(account: Account) -> LoginPasswordChange {
        LoginPasswordChange {
            request: ReauthenticatedRequest::new(account, Verb::Put, LOGIN_PASSWORD_PATH),
            new_password: Secret::default(),
        }
    }
}

#[derive(Debug, Serialize)]
struct AuthRequest {
    #[serde(rename = "Auth")]
    auth: Verifier,
}
