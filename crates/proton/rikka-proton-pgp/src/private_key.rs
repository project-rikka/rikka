use std::fmt;
use std::sync::Arc;

use proton_rpgp::{AccessKeyInfo as _, DEFAULT_PROFILE, DataEncoding, KeyModifier};

use crate::armor::armored_text;
use crate::error::Result;
use crate::identifier::{Fingerprint, KeyId};
use crate::key_flags::KeyFlags;
use crate::locked_key::LockedKey;
use crate::public_key::PublicKey;

#[derive(Clone)]
pub struct PrivateKey {
    inner: Arc<proton_rpgp::PrivateKey>,
}

impl PrivateKey {
    pub fn import_unlocked(data: impl AsRef<[u8]>) -> Result<Self> {
        let inner = proton_rpgp::PrivateKey::import_unlocked(data.as_ref(), DataEncoding::Auto)?;
        Ok(PrivateKey::from_inner(inner))
    }

    pub fn export_unlocked(&self) -> Result<Vec<u8>> {
        Ok(self.inner.export_unlocked(DataEncoding::Unarmored)?)
    }

    pub fn export_unlocked_armored(&self) -> Result<String> {
        let armored = self.inner.export_unlocked(DataEncoding::Armored)?;
        armored_text(armored)
    }

    pub fn lock(&self, passphrase: impl AsRef<[u8]>) -> Result<LockedKey> {
        let locked = self.inner.lock(&DEFAULT_PROFILE, passphrase.as_ref())?;
        Ok(LockedKey::from_inner(locked))
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

    #[must_use]
    pub fn version(&self) -> u8 {
        self.inner.version()
    }

    pub fn with_user_id(&self, name: &str, email: &str) -> Result<Self> {
        let changed = KeyModifier::new_with_profile(&self.inner, &DEFAULT_PROFILE)
            .clear_existing_user_ids()
            .add_user_id(name, email)
            .apply()?;
        Ok(PrivateKey::from_inner(changed))
    }

    pub fn expires_at(&self) -> Result<Option<u64>> {
        self.public_key().expires_at()
    }

    pub fn flags(&self) -> Result<KeyFlags> {
        self.public_key().flags()
    }

    #[must_use]
    pub fn is_expired_at(&self, unix_seconds: u64) -> bool {
        self.public_key().is_expired_at(unix_seconds)
    }

    #[must_use]
    pub fn sha256_fingerprints(&self) -> Vec<Fingerprint> {
        self.public_key().sha256_fingerprints()
    }

    #[must_use]
    pub fn is_forwarding_key(&self) -> bool {
        self.inner.is_forwarding_key(&DEFAULT_PROFILE)
    }

    pub(crate) fn from_inner(inner: proton_rpgp::PrivateKey) -> Self {
        PrivateKey {
            inner: Arc::new(inner),
        }
    }

    pub(crate) fn as_inner(&self) -> &proton_rpgp::PrivateKey {
        &self.inner
    }
}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateKey")
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}
