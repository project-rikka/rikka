use rikka_proton_pgp::Secret;
use serde_json::{Value, json};

use crate::account::Account;
use crate::error::Result;
use crate::news::NewsSubscriptions;
use crate::reauthenticated_request::{ReauthenticatedRequest, Verb};
use crate::recovery_codes::RecoveryCodesRequest;
use crate::settings::UserSettings;
use crate::settings_change::SettingsChange;

const SETTINGS_PATH: &str = "core/v4/settings";

impl Account {
    pub async fn set_username(&self, username: &str) -> Result<()> {
        self.api()
            .put(format!("{SETTINGS_PATH}/username"))
            .json(&json!({ "Username": username }))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn set_locale(&self, locale: &str) -> Result<UserSettings> {
        self.put_setting("locale", json!({ "Locale": locale }))
            .await
    }

    pub async fn set_news(&self, news: &NewsSubscriptions) -> Result<UserSettings> {
        let response: UserSettings = self
            .api()
            .patch(format!("{SETTINGS_PATH}/news"))
            .json(news)
            .send_field_or_default("UserSettings")
            .await?;

        Ok(response)
    }

    pub async fn set_log_auth(&self, level: i32) -> Result<UserSettings> {
        self.put_setting("logauth", json!({ "LogAuth": level }))
            .await
    }

    pub async fn set_density(&self, density: i32) -> Result<UserSettings> {
        self.put_setting("density", json!({ "Density": density }))
            .await
    }

    pub async fn set_week_start(&self, week_start: i32) -> Result<UserSettings> {
        self.put_setting("weekstart", json!({ "WeekStart": week_start }))
            .await
    }

    pub async fn set_date_format(&self, date_format: i32) -> Result<UserSettings> {
        self.put_setting("dateformat", json!({ "DateFormat": date_format }))
            .await
    }

    pub async fn set_time_format(&self, time_format: i32) -> Result<UserSettings> {
        self.put_setting("timeformat", json!({ "TimeFormat": time_format }))
            .await
    }

    pub async fn set_invoice_text(&self, text: &str) -> Result<UserSettings> {
        self.put_setting("invoicetext", json!({ "InvoiceText": text }))
            .await
    }

    pub async fn set_telemetry(&self, enabled: bool) -> Result<UserSettings> {
        self.put_setting("telemetry", json!({ "Telemetry": u8::from(enabled) }))
            .await
    }

    pub async fn set_crash_reports(&self, enabled: bool) -> Result<UserSettings> {
        self.put_setting("crashreports", json!({ "CrashReports": u8::from(enabled) }))
            .await
    }

    pub async fn set_device_recovery(&self, enabled: bool) -> Result<UserSettings> {
        self.put_setting(
            "devicerecovery",
            json!({ "DeviceRecovery": u8::from(enabled) }),
        )
        .await
    }

    pub async fn set_recovery_email_notifications(&self, notify: bool) -> Result<UserSettings> {
        self.put_setting("email/notify", json!({ "Notify": u8::from(notify) }))
            .await
    }

    pub async fn set_recovery_phone_notifications(&self, notify: bool) -> Result<UserSettings> {
        self.put_setting("phone/notify", json!({ "Notify": u8::from(notify) }))
            .await
    }

    pub async fn set_early_access(&self, enabled: bool) -> Result<()> {
        self.api()
            .put(format!("{SETTINGS_PATH}/earlyaccess"))
            .json(&json!({ "EarlyAccess": u8::from(enabled) }))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn mark_welcomed(&self) -> Result<()> {
        self.api()
            .put(format!("{SETTINGS_PATH}/welcome"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn set_fido2_credential_pin(&self, opted_in: bool) -> Result<()> {
        self.api()
            .put(format!("{SETTINGS_PATH}/flags"))
            .json(&json!({ "Fido2CredentialsPinOptIn": u8::from(opted_in) }))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn set_high_security(&self, enabled: bool) -> Result<()> {
        self.set_enabled(format!("{SETTINGS_PATH}/highsecurity"), enabled)
            .await
    }

    pub async fn set_breach_alerts(&self, enabled: bool) -> Result<()> {
        self.set_enabled(format!("{SETTINGS_PATH}/breachalerts"), enabled)
            .await
    }

    pub async fn verify_recovery_email(&self, token: &str) -> Result<UserSettings> {
        self.post_setting("email/verify", json!({ "Token": token }))
            .await
    }

    pub async fn verify_recovery_phone(&self, token: &str) -> Result<UserSettings> {
        self.post_setting("phone/verify", json!({ "Token": token }))
            .await
    }

    pub async fn send_recovery_email_verification(&self) -> Result<()> {
        self.api().post("core/v4/verify/email").send_unit().await?;

        Ok(())
    }

    pub async fn send_recovery_phone_verification(&self) -> Result<()> {
        self.api().post("core/v4/verify/phone").send_unit().await?;

        Ok(())
    }

    pub fn set_recovery_email(&self, email: impl Into<String>) -> SettingsChange {
        SettingsChange::new(
            self.reauthenticated(Verb::Put, "email")
                .with_field("Email", email.into()),
        )
    }

    pub fn set_recovery_email_reset(&self, enabled: bool) -> SettingsChange {
        SettingsChange::new(
            self.reauthenticated(Verb::Put, "email/reset")
                .with_field("Reset", u8::from(enabled)),
        )
    }

    pub fn set_recovery_phone(&self, phone: impl Into<String>) -> SettingsChange {
        SettingsChange::new(
            self.reauthenticated(Verb::Put, "phone")
                .with_field("Phone", phone.into()),
        )
    }

    pub fn set_recovery_phone_reset(&self, enabled: bool) -> SettingsChange {
        SettingsChange::new(
            self.reauthenticated(Verb::Put, "phone/reset")
                .with_field("Reset", u8::from(enabled)),
        )
    }

    pub fn disable_second_factor(&self) -> SettingsChange {
        SettingsChange::new(self.reauthenticated(Verb::Put, "2fa"))
    }

    pub fn disable_totp(&self) -> SettingsChange {
        SettingsChange::new(self.reauthenticated(Verb::Put, "2fa/totp"))
    }

    pub async fn totp_secret(&self) -> Result<Secret> {
        Ok(self
            .api()
            .get(format!("{SETTINGS_PATH}/2fa/totp/secret"))
            .send_field("TOTPSharedSecret")
            .await?)
    }

    pub fn enable_totp(&self, confirmation: impl Into<String>) -> RecoveryCodesRequest {
        RecoveryCodesRequest::new(
            self.reauthenticated(Verb::Post, "2fa/totp")
                .with_field("TOTPConfirmation", confirmation.into()),
        )
    }

    pub fn regenerate_recovery_codes(&self) -> RecoveryCodesRequest {
        RecoveryCodesRequest::new(self.reauthenticated(Verb::Post, "2fa/codes"))
    }

    fn reauthenticated(&self, verb: Verb, path: &str) -> ReauthenticatedRequest {
        ReauthenticatedRequest::new(self.clone(), verb, format!("{SETTINGS_PATH}/{path}"))
    }

    async fn put_setting(&self, path: &str, body: Value) -> Result<UserSettings> {
        let response: UserSettings = self
            .api()
            .put(format!("{SETTINGS_PATH}/{path}"))
            .json(&body)
            .send_field_or_default("UserSettings")
            .await?;

        Ok(response)
    }

    async fn post_setting(&self, path: &str, body: Value) -> Result<UserSettings> {
        let response: UserSettings = self
            .api()
            .post(format!("{SETTINGS_PATH}/{path}"))
            .json(&body)
            .send_field_or_default("UserSettings")
            .await?;

        Ok(response)
    }

    async fn set_enabled(&self, path: String, enabled: bool) -> Result<()> {
        let request = match enabled {
            true => self.api().post(path).json(&json!({})),
            false => self.api().delete(path).json(&json!({})),
        };
        request.send_unit().await?;

        Ok(())
    }
}
