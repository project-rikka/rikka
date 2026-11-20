use std::fmt;

use rikka_proton_pgp::SymmetricKey;

use crate::crypto::tag::EncryptionTag;
use crate::error::{Error, Result};

#[derive(Clone)]
pub struct EncryptionKey(SymmetricKey);

impl EncryptionKey {
    pub const LENGTH: usize = SymmetricKey::LENGTH;

    #[must_use]
    pub fn generate() -> EncryptionKey {
        EncryptionKey(SymmetricKey::generate())
    }

    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<EncryptionKey> {
        let bytes = bytes.as_ref();
        let key = SymmetricKey::from_bytes(bytes).map_err(|_| Error::InvalidKeyLength {
            length: bytes.len(),
        })?;

        Ok(EncryptionKey(key))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn seal(&self, data: impl AsRef<[u8]>, tag: &EncryptionTag) -> Result<Vec<u8>> {
        Ok(self.0.seal(data, tag.aad())?)
    }

    pub fn open(&self, sealed: impl AsRef<[u8]>, tag: &EncryptionTag) -> Result<Vec<u8>> {
        self.0
            .open(sealed, tag.aad())
            .map_err(|_| Error::Undecryptable)
    }
}

impl fmt::Debug for EncryptionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptionKey")
            .finish_non_exhaustive()
    }
}
