use std::time::Duration;

use jiff::Timestamp;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    Stale,
    Expired,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Certificate {
    #[serde(rename = "Certificate")]
    pub certificate: String,
    #[serde(
        rename = "ExpirationTime",
        deserialize_with = "rikka_proton_api::unix_seconds::deserialize"
    )]
    pub expires_at: Timestamp,
    #[serde(
        rename = "RefreshTime",
        deserialize_with = "rikka_proton_api::unix_seconds::deserialize"
    )]
    pub refreshes_at: Timestamp,
    #[serde(rename = "SerialNumber", default)]
    pub serial_number: Option<String>,
    #[serde(rename = "ClientKeyFingerprint", default)]
    pub client_key_fingerprint: Option<String>,
    #[serde(rename = "ClientKey", default)]
    pub client_key: Option<String>,
    #[serde(rename = "Mode", default)]
    pub mode: Option<String>,
    #[serde(rename = "DeviceName", default)]
    pub device_name: Option<String>,
    #[serde(rename = "ServerPublicKeyMode", default)]
    pub server_public_key_mode: Option<String>,
    #[serde(rename = "ServerPublicKey", default)]
    pub server_public_key: Option<String>,
}

impl Freshness {
    #[must_use]
    pub fn is_expired(self) -> bool {
        self == Freshness::Expired
    }

    #[must_use]
    pub fn needs_refresh(self) -> bool {
        self != Freshness::Fresh
    }
}

impl Certificate {
    #[must_use]
    pub fn freshness_at(&self, now: Timestamp) -> Freshness {
        if now >= self.expires_at {
            return Freshness::Expired;
        }
        match now >= self.refreshes_at {
            true => Freshness::Stale,
            false => Freshness::Fresh,
        }
    }

    #[must_use]
    pub fn freshness(&self) -> Freshness {
        self.freshness_at(Timestamp::now())
    }

    #[must_use]
    pub fn time_until_refresh_at(&self, now: Timestamp) -> Duration {
        remaining(self.refreshes_at, now)
    }

    #[must_use]
    pub fn time_until_expiry_at(&self, now: Timestamp) -> Duration {
        remaining(self.expires_at, now)
    }

    #[must_use]
    pub fn matches(&self, fingerprint: &str) -> bool {
        self.client_key_fingerprint.as_deref() == Some(fingerprint)
    }
}

fn remaining(deadline: Timestamp, now: Timestamp) -> Duration {
    let milliseconds = deadline.as_millisecond() - now.as_millisecond();
    Duration::from_millis(milliseconds.max(0).unsigned_abs())
}
