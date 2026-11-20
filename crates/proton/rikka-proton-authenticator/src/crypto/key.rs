use std::fmt;

use rikka_proton_pgp::SymmetricKey;

use crate::error::{Error, Result};

#[derive(Clone)]
pub struct AuthenticatorKey(SymmetricKey);

impl AuthenticatorKey {
    #[must_use]
    pub fn generate() -> AuthenticatorKey {
        AuthenticatorKey(SymmetricKey::generate())
    }

    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<AuthenticatorKey> {
        let key = SymmetricKey::from_bytes(bytes).map_err(|_| Error::InvalidKeyLength)?;

        Ok(AuthenticatorKey(key))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub(crate) fn material(&self) -> &SymmetricKey {
        &self.0
    }
}

impl fmt::Debug for AuthenticatorKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatorKey")
            .finish_non_exhaustive()
    }
}
