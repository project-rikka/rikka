use std::fmt;

use proton_rpgp::DEFAULT_PROFILE;
use proton_rpgp::pgp::crypto::sym::SymmetricKeyAlgorithm;

use crate::error::{Error, Result};

#[derive(Clone)]
pub struct SessionKey(proton_rpgp::SessionKey);

impl SessionKey {
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self> {
        let bytes = bytes.as_ref();
        let algorithm = match bytes.len() {
            16 => SymmetricKeyAlgorithm::AES128,
            24 => SymmetricKeyAlgorithm::AES192,
            32 => SymmetricKeyAlgorithm::AES256,
            _ => return Err(Error::Invalid("session key must be 16, 24 or 32 bytes")),
        };
        Ok(SessionKey(proton_rpgp::SessionKey::new(bytes, algorithm)))
    }

    #[must_use]
    pub fn generate() -> Self {
        SessionKey(proton_rpgp::SessionKey::generate_for_seipdv1(
            SymmetricKeyAlgorithm::AES256,
            &DEFAULT_PROFILE,
        ))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    pub(crate) fn from_inner(inner: proton_rpgp::SessionKey) -> Self {
        SessionKey(inner)
    }

    pub(crate) fn as_inner(&self) -> &proton_rpgp::SessionKey {
        &self.0
    }
}

impl fmt::Debug for SessionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionKey")
            .field("bytes", &self.as_bytes().len())
            .finish_non_exhaustive()
    }
}
