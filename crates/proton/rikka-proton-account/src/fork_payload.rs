use std::fmt;

use rikka_proton_pgp::{NonceLength, SecretBytes};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::fork_key::ForkKey;

#[must_use]
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ForkPayload {
    #[serde(
        rename = "keyPassword",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    key_password: Option<String>,
}

impl ForkPayload {
    pub fn new() -> ForkPayload {
        ForkPayload::default()
    }

    pub fn with_key_password(mut self, key_password: impl Into<String>) -> ForkPayload {
        self.key_password = Some(key_password.into());
        self
    }

    #[must_use]
    pub fn key_password(&self) -> Option<&str> {
        self.key_password.as_deref()
    }

    pub fn seal(&self, key: &ForkKey) -> Result<String> {
        let plaintext =
            SecretBytes::new(serde_json::to_vec(self).map_err(Error::InvalidForkPayloadJson)?);
        let sealed = key.symmetric_key().seal(plaintext.expose(), b"")?;

        Ok(rikka_proton_api::base64::encode(sealed))
    }

    pub fn open(key: &ForkKey, sealed: impl AsRef<str>) -> Result<ForkPayload> {
        let sealed = rikka_proton_api::base64::decode(sealed.as_ref())
            .map_err(|_| Error::MalformedForkPayload)?;
        if sealed.len() <= NonceLength::Long.bytes() {
            return Err(Error::MalformedForkPayload);
        }

        let plaintext = SecretBytes::new(
            key.symmetric_key()
                .open(&sealed, b"")
                .map_err(|_| Error::ForkPayloadDecryption)?,
        );

        serde_json::from_slice(plaintext.expose()).map_err(Error::InvalidForkPayloadJson)
    }
}

impl fmt::Debug for ForkPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ForkPayload")
            .field("key_password", &self.key_password.is_some())
            .finish()
    }
}
