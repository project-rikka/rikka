use std::fmt;
use std::sync::Arc;

use proton_rpgp::{AccessKeyInfo as _, DataEncoding};

use crate::armor::armored_text;
use crate::error::Result;
use crate::identifier::{Fingerprint, KeyId};
use crate::private_key::PrivateKey;
use crate::public_key::PublicKey;

#[derive(Clone)]
pub struct LockedKey {
    inner: Arc<proton_rpgp::LockedPrivateKey>,
}

impl LockedKey {
    pub fn import(data: impl AsRef<[u8]>) -> Result<Self> {
        let inner = proton_rpgp::LockedPrivateKey::import(data.as_ref(), DataEncoding::Auto)?;
        Ok(LockedKey::from_inner(inner))
    }

    pub fn import_many(data: impl AsRef<[u8]>) -> Result<Vec<Self>> {
        let keys = proton_rpgp::LockedPrivateKey::import_many(data.as_ref())?;
        Ok(keys.into_iter().map(LockedKey::from_inner).collect())
    }

    pub fn unlock(&self, passphrase: impl AsRef<[u8]>) -> Result<PrivateKey> {
        let passphrase = if self.is_locked() {
            passphrase.as_ref()
        } else {
            &[]
        };
        let unlocked = self.inner.unlock(passphrase)?;
        Ok(PrivateKey::from_inner(unlocked))
    }

    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.inner.is_locked()
    }

    pub fn export(&self) -> Result<Vec<u8>> {
        Ok(self.inner.export(DataEncoding::Unarmored)?)
    }

    pub fn export_armored(&self) -> Result<String> {
        let armored = self.inner.export(DataEncoding::Armored)?;
        armored_text(armored)
    }

    #[must_use]
    pub fn public_key(&self) -> PublicKey {
        PublicKey::from_inner(proton_rpgp::PublicKey::from(self.inner.as_ref()))
    }

    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        self.inner.fingerprint().into()
    }

    #[must_use]
    pub fn key_id(&self) -> KeyId {
        self.inner.key_id().into()
    }

    pub(crate) fn from_inner(inner: proton_rpgp::LockedPrivateKey) -> Self {
        LockedKey {
            inner: Arc::new(inner),
        }
    }
}

impl fmt::Debug for LockedKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LockedKey")
            .field("fingerprint", &self.fingerprint())
            .field("locked", &self.is_locked())
            .finish_non_exhaustive()
    }
}
