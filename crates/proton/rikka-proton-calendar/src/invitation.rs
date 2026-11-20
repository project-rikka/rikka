use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::calendar::Permissions;
use crate::identifier::{CalendarId, InvitationId, PassphraseId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
pub enum InvitationStatus {
    #[default]
    Pending,
    Accepted,
    Rejected,
}

impl From<i32> for InvitationStatus {
    fn from(value: i32) -> InvitationStatus {
        match value {
            1 => InvitationStatus::Accepted,
            2 => InvitationStatus::Rejected,
            _ => InvitationStatus::Pending,
        }
    }
}

impl From<InvitationStatus> for i32 {
    fn from(status: InvitationStatus) -> i32 {
        match status {
            InvitationStatus::Pending => 0,
            InvitationStatus::Accepted => 1,
            InvitationStatus::Rejected => 2,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvitedCalendar {
    #[serde(rename = "Color", default)]
    color: String,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "SenderEmail", default)]
    sender_email: String,
}

impl InvitedCalendar {
    #[must_use]
    pub fn color(&self) -> &str {
        &self.color
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn sender_email(&self) -> &str {
        &self.sender_email
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarInvitation {
    #[serde(rename = "CalendarInvitationID", default)]
    id: InvitationId,
    #[serde(rename = "CalendarID", default)]
    calendar_id: CalendarId,
    #[serde(rename = "Calendar", default)]
    calendar: InvitedCalendar,
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "Passphrase", default)]
    passphrase: String,
    #[serde(rename = "PassphraseID", default)]
    passphrase_id: PassphraseId,
    #[serde(rename = "Permissions", default)]
    permissions: Permissions,
    #[serde(rename = "Status", default)]
    status: InvitationStatus,
    #[serde(rename = "Signature", default)]
    signature: String,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(
        rename = "ExpirationTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    expiration_time: Timestamp,
}

impl CalendarInvitation {
    #[must_use]
    pub fn id(&self) -> &InvitationId {
        &self.id
    }

    #[must_use]
    pub fn calendar_id(&self) -> &CalendarId {
        &self.calendar_id
    }

    #[must_use]
    pub fn calendar(&self) -> &InvitedCalendar {
        &self.calendar
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn armored_passphrase(&self) -> &str {
        &self.passphrase
    }

    #[must_use]
    pub fn passphrase_id(&self) -> &PassphraseId {
        &self.passphrase_id
    }

    #[must_use]
    pub fn permissions(&self) -> Permissions {
        self.permissions
    }

    #[must_use]
    pub fn status(&self) -> InvitationStatus {
        self.status
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
    pub fn expiration_time(&self) -> Timestamp {
        self.expiration_time
    }

    #[must_use]
    pub fn is_expired_at(&self, at: Timestamp) -> bool {
        self.expiration_time != Timestamp::UNIX_EPOCH && at >= self.expiration_time
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HolidaySessionKey {
    #[serde(rename = "Key", default)]
    key: String,
    #[serde(rename = "Algorithm", default)]
    algorithm: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HolidayCalendar {
    #[serde(rename = "CalendarID", default)]
    calendar_id: CalendarId,
    #[serde(rename = "Country", default)]
    country: String,
    #[serde(rename = "CountryCode", default)]
    country_code: String,
    #[serde(rename = "Hidden", default)]
    hidden: bool,
    #[serde(rename = "Language", default)]
    language: String,
    #[serde(rename = "LanguageCode", default)]
    language_code: String,
    #[serde(rename = "Passphrase", default)]
    passphrase: String,
    #[serde(rename = "SessionKey", default)]
    session_key: HolidaySessionKey,
    #[serde(rename = "Timezones", default)]
    timezones: Vec<String>,
}

impl HolidayCalendar {
    #[must_use]
    pub fn calendar_id(&self) -> &CalendarId {
        &self.calendar_id
    }

    #[must_use]
    pub fn country(&self) -> &str {
        &self.country
    }

    #[must_use]
    pub fn country_code(&self) -> &str {
        &self.country_code
    }

    #[must_use]
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    #[must_use]
    pub fn language(&self) -> &str {
        &self.language
    }

    #[must_use]
    pub fn language_code(&self) -> &str {
        &self.language_code
    }

    #[must_use]
    pub fn timezones(&self) -> &[String] {
        &self.timezones
    }

    #[must_use]
    pub fn passphrase(&self) -> &str {
        &self.passphrase
    }

    #[must_use]
    pub fn session_key_base64(&self) -> &str {
        &self.session_key.key
    }

    #[must_use]
    pub fn session_key_algorithm(&self) -> &str {
        &self.session_key.algorithm
    }
}
