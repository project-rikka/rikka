use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::identifier::CalendarId;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
#[non_exhaustive]
pub enum SubscriptionStatus {
    #[default]
    Ok,
    InvalidIcs,
    TooBig,
    Synchronizing,
    MissingPrimaryKey,
    HttpFailed(i32),
    InternalUrlNotFound,
    InternalUndecryptable,
    InvalidUrl,
    Other(i32),
}

impl From<i32> for SubscriptionStatus {
    fn from(value: i32) -> SubscriptionStatus {
        match value {
            0 => SubscriptionStatus::Ok,
            2 => SubscriptionStatus::InvalidIcs,
            6 => SubscriptionStatus::TooBig,
            7 => SubscriptionStatus::Synchronizing,
            8 => SubscriptionStatus::MissingPrimaryKey,
            20..=26 => SubscriptionStatus::HttpFailed(value),
            27 => SubscriptionStatus::InternalUrlNotFound,
            28 => SubscriptionStatus::InternalUndecryptable,
            30 => SubscriptionStatus::InvalidUrl,
            other => SubscriptionStatus::Other(other),
        }
    }
}

impl From<SubscriptionStatus> for i32 {
    fn from(status: SubscriptionStatus) -> i32 {
        match status {
            SubscriptionStatus::Ok => 0,
            SubscriptionStatus::InvalidIcs => 2,
            SubscriptionStatus::TooBig => 6,
            SubscriptionStatus::Synchronizing => 7,
            SubscriptionStatus::MissingPrimaryKey => 8,
            SubscriptionStatus::HttpFailed(code) | SubscriptionStatus::Other(code) => code,
            SubscriptionStatus::InternalUrlNotFound => 27,
            SubscriptionStatus::InternalUndecryptable => 28,
            SubscriptionStatus::InvalidUrl => 30,
        }
    }
}

impl SubscriptionStatus {
    #[must_use]
    pub fn is_synced(self) -> bool {
        self == SubscriptionStatus::Ok
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    #[serde(rename = "CalendarID", default)]
    calendar_id: CalendarId,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(
        rename = "LastUpdateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    last_update_time: Timestamp,
    #[serde(rename = "Status", default)]
    status: SubscriptionStatus,
    #[serde(rename = "URL", default)]
    url: String,
}

impl Subscription {
    #[must_use]
    pub fn calendar_id(&self) -> &CalendarId {
        &self.calendar_id
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn last_update_time(&self) -> Timestamp {
        self.last_update_time
    }

    #[must_use]
    pub fn status(&self) -> SubscriptionStatus {
        self.status
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
}
