use std::fmt;

use rikka_proton_pgp::{NonceLength, SymmetricKey};

use crate::error::{Error, Result};

#[derive(Clone)]
pub struct ForkKey(SymmetricKey);

impl ForkKey {
    #[must_use]
    pub fn generate() -> ForkKey {
        ForkKey(SymmetricKey::generate().with_nonce(NonceLength::Long))
    }

    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<ForkKey> {
        let key = SymmetricKey::from_bytes(bytes).map_err(|_| Error::ForkKeyLength)?;
        Ok(ForkKey(key.with_nonce(NonceLength::Long)))
    }

    pub fn from_base64(encoded: impl AsRef<str>) -> Result<ForkKey> {
        let bytes = rikka_proton_api::base64::decode(encoded.as_ref())
            .map_err(|_| Error::MalformedForkKey)?;
        ForkKey::from_bytes(bytes)
    }

    #[must_use]
    pub fn to_base64(&self) -> String {
        rikka_proton_api::base64::encode(self.as_bytes())
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub(crate) fn symmetric_key(&self) -> &SymmetricKey {
        &self.0
    }
}

impl fmt::Debug for ForkKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("ForkKey").finish_non_exhaustive()
    }
}
