use std::fmt;

use rikka_proton_pgp::Secret;
use serde::Deserialize;

use crate::error::Result;
use crate::reauthenticated_request::ReauthenticatedRequest;
use crate::reauthentication::reauthenticated;
use crate::settings::UserSettings;

#[must_use]
#[derive(Debug)]
pub struct RecoveryCodesRequest {
    request: ReauthenticatedRequest,
}

reauthenticated!(RecoveryCodesRequest, request.reauthentication);

impl RecoveryCodesRequest {
    pub fn keep_password_scope(mut self) -> Self {
        self.request.keep_password_scope();
        self
    }

    pub async fn execute(self) -> Result<RecoveryCodes> {
        self.request.send().await
    }

    pub(crate) fn new(request: ReauthenticatedRequest) -> RecoveryCodesRequest {
        RecoveryCodesRequest { request }
    }
}

#[derive(Clone, Deserialize)]
pub struct RecoveryCodes {
    #[serde(rename = "TwoFactorRecoveryCodes", default)]
    codes: Vec<Secret>,
    #[serde(rename = "UserSettings", default)]
    settings: UserSettings,
}

impl RecoveryCodes {
    #[must_use]
    pub fn codes(&self) -> &[Secret] {
        &self.codes
    }

    #[must_use]
    pub fn settings(&self) -> &UserSettings {
        &self.settings
    }
}

impl fmt::Debug for RecoveryCodes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveryCodes")
            .field("codes", &self.codes.len())
            .finish_non_exhaustive()
    }
}
