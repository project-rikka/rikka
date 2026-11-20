use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::fido2::RegisteredKey;
use crate::password_mode::PasswordMode;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserSettings {
    #[serde(rename = "Email", default, skip_serializing_if = "Option::is_none")]
    email: Option<RecoverySetting>,
    #[serde(rename = "Phone", default, skip_serializing_if = "Option::is_none")]
    phone: Option<RecoverySetting>,
    #[serde(rename = "Password", default)]
    password: PasswordSettings,
    #[serde(rename = "2FA", default, skip_serializing_if = "Option::is_none")]
    second_factor: Option<SecondFactorSettings>,
    #[serde(rename = "News", default)]
    news: i32,
    #[serde(rename = "Locale", default)]
    locale: String,
    #[serde(rename = "LogAuth", default)]
    log_auth: i32,
    #[serde(rename = "Density", default)]
    density: i32,
    #[serde(rename = "WeekStart", default)]
    week_start: i32,
    #[serde(rename = "DateFormat", default)]
    date_format: i32,
    #[serde(rename = "TimeFormat", default)]
    time_format: i32,
    #[serde(rename = "EarlyAccess", default, with = "rikka_proton_api::flag")]
    early_access: bool,
    #[serde(rename = "DeviceRecovery", default, with = "rikka_proton_api::flag")]
    device_recovery: bool,
    #[serde(rename = "Telemetry", default, with = "rikka_proton_api::flag")]
    telemetry: bool,
    #[serde(rename = "CrashReports", default, with = "rikka_proton_api::flag")]
    crash_reports: bool,
    #[serde(
        rename = "SessionAccountRecovery",
        default,
        with = "rikka_proton_api::flag"
    )]
    session_account_recovery: bool,
    #[serde(rename = "Flags", default)]
    flags: SettingsFlags,
}

impl UserSettings {
    #[must_use]
    pub fn email(&self) -> Option<&RecoverySetting> {
        self.email.as_ref()
    }

    #[must_use]
    pub fn phone(&self) -> Option<&RecoverySetting> {
        self.phone.as_ref()
    }

    #[must_use]
    pub fn password(&self) -> &PasswordSettings {
        &self.password
    }

    #[must_use]
    pub fn second_factor(&self) -> Option<&SecondFactorSettings> {
        self.second_factor.as_ref()
    }

    #[must_use]
    pub fn news(&self) -> i32 {
        self.news
    }

    #[must_use]
    pub fn locale(&self) -> &str {
        &self.locale
    }

    #[must_use]
    pub fn logs_authentication(&self) -> bool {
        self.log_auth != 0
    }

    #[must_use]
    pub fn density(&self) -> i32 {
        self.density
    }

    #[must_use]
    pub fn week_start(&self) -> i32 {
        self.week_start
    }

    #[must_use]
    pub fn date_format(&self) -> i32 {
        self.date_format
    }

    #[must_use]
    pub fn time_format(&self) -> i32 {
        self.time_format
    }

    #[must_use]
    pub fn has_early_access(&self) -> bool {
        self.early_access
    }

    #[must_use]
    pub fn has_device_recovery(&self) -> bool {
        self.device_recovery
    }

    #[must_use]
    pub fn sends_telemetry(&self) -> bool {
        self.telemetry
    }

    #[must_use]
    pub fn sends_crash_reports(&self) -> bool {
        self.crash_reports
    }

    #[must_use]
    pub fn has_session_account_recovery(&self) -> bool {
        self.session_account_recovery
    }

    #[must_use]
    pub fn flags(&self) -> &SettingsFlags {
        &self.flags
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsFlags {
    #[serde(rename = "Welcomed", default)]
    welcomed: i32,
    #[serde(rename = "SupportPgpV6Keys", default)]
    supports_v6_keys: i32,
    #[serde(rename = "EdmOptOut", default)]
    edm_opt_out: i32,
    #[serde(rename = "PasswordReminderOptOut", default)]
    password_reminder_opt_out: i32,
    #[serde(rename = "Fido2CredentialsPinOptIn", default)]
    fido2_credentials_pin_opt_in: i32,
}

impl SettingsFlags {
    #[must_use]
    pub fn is_welcomed(&self) -> bool {
        self.welcomed != 0
    }

    #[must_use]
    pub fn supports_v6_keys(&self) -> bool {
        self.supports_v6_keys != 0
    }

    #[must_use]
    pub fn opted_out_of_email_marketing(&self) -> bool {
        self.edm_opt_out != 0
    }

    #[must_use]
    pub fn opted_out_of_password_reminders(&self) -> bool {
        self.password_reminder_opt_out != 0
    }

    #[must_use]
    pub fn opted_into_fido2_credential_pin(&self) -> bool {
        self.fido2_credentials_pin_opt_in != 0
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecoverySetting {
    #[serde(rename = "Value", default, skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(rename = "Status", default)]
    status: i32,
    #[serde(rename = "Notify", default, with = "rikka_proton_api::flag")]
    notify: bool,
    #[serde(rename = "Reset", default, with = "rikka_proton_api::flag")]
    reset: bool,
}

impl RecoverySetting {
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    #[must_use]
    pub fn status(&self) -> i32 {
        self.status
    }

    #[must_use]
    pub fn notifies(&self) -> bool {
        self.notify
    }

    #[must_use]
    pub fn resets(&self) -> bool {
        self.reset
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PasswordSettings {
    #[serde(rename = "Mode", default)]
    mode: PasswordMode,
    #[serde(
        rename = "ExpirationTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    expiration_time: Option<Timestamp>,
}

impl PasswordSettings {
    #[must_use]
    pub fn mode(&self) -> PasswordMode {
        self.mode
    }

    #[must_use]
    pub fn expiration_time(&self) -> Option<Timestamp> {
        self.expiration_time
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecondFactorSettings {
    #[serde(rename = "Enabled", default)]
    enabled: i32,
    #[serde(rename = "Allowed", default)]
    allowed: i32,
    #[serde(
        rename = "ExpirationTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    expiration_time: Option<Timestamp>,
    #[serde(rename = "RegisteredKeys", default)]
    registered_keys: Vec<RegisteredKey>,
}

impl SecondFactorSettings {
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled != 0
    }

    #[must_use]
    pub fn enabled(&self) -> i32 {
        self.enabled
    }

    #[must_use]
    pub fn allowed(&self) -> i32 {
        self.allowed
    }

    #[must_use]
    pub fn expiration_time(&self) -> Option<Timestamp> {
        self.expiration_time
    }

    #[must_use]
    pub fn registered_keys(&self) -> &[RegisteredKey] {
        &self.registered_keys
    }
}
