use rikka_proton_account::UserKeys;
use rikka_proton_pgp::{Pgp, PublicKey, SignatureStatus};

use crate::api::ShareKeyResponse;
use crate::crypto::key::EncryptionKey;
use crate::crypto::rotated::ShareKey;
use crate::crypto::{decode_base64, encode_base64};
use crate::error::{Error, Result};
use crate::identifier::ShareId;

impl EncryptionKey {
    pub fn wrap(&self, pgp: &Pgp, keys: &UserKeys) -> Result<String> {
        let message = pgp
            .encrypt()
            .to(keys.signer().public_key())
            .signed_by(keys.signer().clone())
            .message(self.as_bytes())?;

        Ok(encode_base64(message))
    }

    pub fn unwrap(
        pgp: &Pgp,
        keys: &UserKeys,
        share: &ShareId,
        message: impl AsRef<[u8]>,
    ) -> Result<EncryptionKey> {
        let decrypted = pgp
            .decrypt()
            .with_ring(keys.ring())
            .verified_by_ring(keys.ring())
            .message(message)?;

        match decrypted.signature() {
            SignatureStatus::Valid { .. } => {}
            SignatureStatus::UnknownKey { key_id } => {
                tracing::warn!(
                    share = %share,
                    key = ?key_id,
                    "share key signed by unknown key"
                );
            }
            status => {
                return Err(Error::InvalidShareKeySignature {
                    share: share.clone(),
                    status: status.clone(),
                });
            }
        }

        EncryptionKey::from_bytes(decrypted.into_data())
    }

    pub fn unwrap_from_group(
        pgp: &Pgp,
        keys: &UserKeys,
        share: &ShareId,
        group_keys: &[PublicKey],
        message: impl AsRef<[u8]>,
    ) -> Result<EncryptionKey> {
        let decrypted = pgp
            .decrypt()
            .with_ring(keys.ring())
            .allow_forwarding()
            .verified_by_keys(group_keys.iter().cloned())
            .message(message)?;

        if !decrypted.signature().is_valid() {
            return Err(Error::InvalidShareKeySignature {
                share: share.clone(),
                status: decrypted.signature().clone(),
            });
        }

        EncryptionKey::from_bytes(decrypted.into_data())
    }
}

impl ShareKey {
    pub fn decrypt(
        pgp: &Pgp,
        keys: &UserKeys,
        share: &ShareId,
        key: &ShareKeyResponse,
    ) -> Result<ShareKey> {
        let message = decode_base64("share key", key.key())?;

        Ok(ShareKey::new(
            key.rotation(),
            EncryptionKey::unwrap(pgp, keys, share, message)?,
        ))
    }

    pub fn decrypt_from_group(
        pgp: &Pgp,
        keys: &UserKeys,
        share: &ShareId,
        group_keys: &[PublicKey],
        key: &ShareKeyResponse,
    ) -> Result<ShareKey> {
        let message = decode_base64("share key", key.key())?;

        Ok(ShareKey::new(
            key.rotation(),
            EncryptionKey::unwrap_from_group(pgp, keys, share, group_keys, message)?,
        ))
    }
}
