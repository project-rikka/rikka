use std::fmt;
use std::sync::Arc;

use proton_rpgp::key::selection::PublicKeySelectionExt as _;
use proton_rpgp::pgp::packet::Signature;
use proton_rpgp::pgp::types::KeyDetails as _;
use proton_rpgp::{AccessKeyInfo as _, CheckUnixTime, DEFAULT_PROFILE, DataEncoding};

use crate::armor::armored_text;
use crate::error::{Error, Result};
use crate::identifier::{Fingerprint, KeyId};
use crate::key_editing;
use crate::key_flags::KeyFlags;

#[derive(Clone)]
pub struct PublicKey {
    inner: Arc<proton_rpgp::PublicKey>,
}

impl PublicKey {
    pub fn import(data: impl AsRef<[u8]>) -> Result<Self> {
        let inner = proton_rpgp::PublicKey::import(data.as_ref(), DataEncoding::Auto)?;
        Ok(PublicKey::from_inner(inner))
    }

    pub fn export(&self) -> Result<Vec<u8>> {
        Ok(self.inner.export(DataEncoding::Unarmored)?)
    }

    pub fn export_armored(&self) -> Result<String> {
        let armored = self.inner.export(DataEncoding::Armored)?;
        armored_text(armored)
    }

    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        self.inner.fingerprint().into()
    }

    #[must_use]
    pub fn fingerprints(&self) -> Vec<Fingerprint> {
        self.inner
            .fingerprints()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    #[must_use]
    pub fn sha256_fingerprints(&self) -> Vec<Fingerprint> {
        self.inner
            .fingerprints_sha256()
            .into_iter()
            .map(|fingerprint| Fingerprint::from(fingerprint.to_hex()))
            .collect()
    }

    #[must_use]
    pub fn key_id(&self) -> KeyId {
        self.inner.key_id().into()
    }

    #[must_use]
    pub fn key_ids(&self) -> Vec<KeyId> {
        self.inner.key_ids().into_iter().map(Into::into).collect()
    }

    #[must_use]
    pub fn version(&self) -> u8 {
        self.inner.version()
    }

    #[must_use]
    pub fn user_ids(&self) -> Vec<String> {
        self.inner
            .as_signed_public_key()
            .details
            .users
            .iter()
            .map(|user| String::from_utf8_lossy(user.id.id()).into_owned())
            .collect()
    }

    #[must_use]
    pub fn created_at(&self) -> u64 {
        u64::from(
            self.inner
                .as_signed_public_key()
                .primary_key()
                .created_at()
                .as_secs(),
        )
    }

    pub fn expires_at(&self) -> Result<Option<u64>> {
        let lifetime = self.primary_self_signature()?.key_expiration_time();
        Ok(lifetime.map(|lifetime| {
            self.created_at()
                .saturating_add(u64::from(lifetime.as_secs()))
        }))
    }

    pub fn flags(&self) -> Result<KeyFlags> {
        Ok(self.primary_self_signature()?.key_flags().into())
    }

    fn primary_self_signature(&self) -> Result<&Signature> {
        self.inner
            .as_signed_public_key()
            .primary_self_signature(CheckUnixTime::disable(), &DEFAULT_PROFILE)
            .map_err(|error| Error::KeyUnusable(Box::new(error)))
    }

    #[must_use]
    pub fn is_expired_at(&self, unix_seconds: u64) -> bool {
        self.inner
            .is_expired(&DEFAULT_PROFILE, CheckUnixTime::new(unix_seconds))
    }

    #[must_use]
    pub fn is_revoked_at(&self, unix_seconds: u64) -> bool {
        self.inner
            .is_revoked(&DEFAULT_PROFILE, CheckUnixTime::new(unix_seconds))
    }

    #[must_use]
    pub fn can_encrypt_at(&self, unix_seconds: u64) -> bool {
        self.inner
            .check_can_encrypt(&DEFAULT_PROFILE, CheckUnixTime::new(unix_seconds))
            .is_ok()
    }

    #[must_use]
    pub fn can_verify_at(&self, unix_seconds: u64) -> bool {
        self.inner
            .check_can_verify(&DEFAULT_PROFILE, CheckUnixTime::new(unix_seconds))
            .is_ok()
    }

    pub fn with_revocation_certificate(&self, certificate: impl AsRef<[u8]>) -> Result<Self> {
        let revoked = key_editing::with_revocation_certificate(&self.inner, certificate.as_ref())?;
        Ok(PublicKey::from_inner(revoked))
    }

    pub(crate) fn from_inner(inner: proton_rpgp::PublicKey) -> Self {
        PublicKey {
            inner: Arc::new(inner),
        }
    }

    pub(crate) fn as_inner(&self) -> &proton_rpgp::PublicKey {
        &self.inner
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicKey")
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}
