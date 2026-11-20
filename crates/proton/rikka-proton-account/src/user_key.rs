use rikka_proton_pgp::LockedKey;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::identifier::KeyId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserKey {
    #[serde(rename = "ID")]
    id: KeyId,
    #[serde(rename = "Version", default)]
    version: i32,
    #[serde(rename = "PrivateKey")]
    private_key: String,
    #[serde(
        rename = "Fingerprint",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    fingerprint: Option<String>,
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
    #[serde(
        rename = "RecoverySecret",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    recovery_secret: Option<String>,
    #[serde(
        rename = "RecoverySecretSignature",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    recovery_secret_signature: Option<String>,
}

impl UserKey {
    #[must_use]
    pub fn id(&self) -> &KeyId {
        &self.id
    }

    #[must_use]
    pub fn version(&self) -> i32 {
        self.version
    }

    #[must_use]
    pub fn private_key(&self) -> &str {
        &self.private_key
    }

    #[must_use]
    pub fn fingerprint(&self) -> Option<&str> {
        self.fingerprint.as_deref()
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
    pub fn recovery_secret(&self) -> Option<&str> {
        self.recovery_secret.as_deref()
    }

    #[must_use]
    pub fn recovery_secret_signature(&self) -> Option<&str> {
        self.recovery_secret_signature.as_deref()
    }

    pub fn to_locked_key(&self) -> Result<LockedKey> {
        Ok(LockedKey::import(&self.private_key)?)
    }
}
