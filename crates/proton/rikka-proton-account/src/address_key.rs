use rikka_proton_pgp::LockedKey;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::identifier::KeyId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressKey {
    #[serde(rename = "ID")]
    id: KeyId,
    #[serde(rename = "Version", default)]
    version: i32,
    #[serde(rename = "Flags", default)]
    flags: i32,
    #[serde(rename = "PrivateKey")]
    private_key: String,
    #[serde(rename = "Token", default, skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(rename = "Signature", default, skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(
        rename = "Fingerprint",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    fingerprint: Option<String>,
    #[serde(
        rename = "Fingerprints",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    fingerprints: Option<Vec<String>>,
    #[serde(
        rename = "Activation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    activation: Option<String>,
    #[serde(rename = "Primary", default, with = "rikka_proton_api::flag")]
    primary: bool,
    #[serde(
        rename = "Active",
        default = "rikka_proton_api::flag::default_true",
        with = "rikka_proton_api::flag"
    )]
    active: bool,
}

impl AddressKey {
    #[must_use]
    pub fn id(&self) -> &KeyId {
        &self.id
    }

    #[must_use]
    pub fn version(&self) -> i32 {
        self.version
    }

    #[must_use]
    pub fn flags(&self) -> i32 {
        self.flags
    }

    #[must_use]
    pub fn private_key(&self) -> &str {
        &self.private_key
    }

    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    #[must_use]
    pub fn signature(&self) -> Option<&str> {
        self.signature.as_deref()
    }

    #[must_use]
    pub fn fingerprint(&self) -> Option<&str> {
        self.fingerprint.as_deref()
    }

    #[must_use]
    pub fn fingerprints(&self) -> &[String] {
        self.fingerprints.as_deref().unwrap_or_default()
    }

    #[must_use]
    pub fn activation(&self) -> Option<&str> {
        self.activation.as_deref()
    }

    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.primary
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[must_use]
    pub fn is_migrated(&self) -> bool {
        self.token.is_some() && self.signature.is_some()
    }

    pub fn to_locked_key(&self) -> Result<LockedKey> {
        Ok(LockedKey::import(&self.private_key)?)
    }
}
