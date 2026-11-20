use crate::error::Result;
use crate::reauthenticated_request::ReauthenticatedRequest;
use crate::reauthentication::reauthenticated;
use crate::settings::UserSettings;

#[must_use]
#[derive(Debug)]
pub struct SettingsChange {
    request: ReauthenticatedRequest,
}

reauthenticated!(SettingsChange, request.reauthentication);

impl SettingsChange {
    pub fn keep_password_scope(mut self) -> Self {
        self.request.keep_password_scope();
        self
    }

    pub async fn execute(self) -> Result<UserSettings> {
        self.request.send_field_or_default("UserSettings").await
    }

    pub(crate) fn new(request: ReauthenticatedRequest) -> SettingsChange {
        SettingsChange { request }
    }
}
