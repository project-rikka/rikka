use std::fmt;

use rikka_proton_account::AddressId;
use rikka_proton_pgp::{PrivateKey, SessionKey, SignatureStatus};

use crate::identifier::{LinkId, ShareId, VolumeId};

#[derive(Clone)]
pub struct ShareKeys {
    id: ShareId,
    volume_id: VolumeId,
    root_id: LinkId,
    address_id: AddressId,
    key: PrivateKey,
    session_key: SessionKey,
    signature: SignatureStatus,
}

impl ShareKeys {
    #[must_use]
    pub fn id(&self) -> &ShareId {
        &self.id
    }

    #[must_use]
    pub fn volume_id(&self) -> &VolumeId {
        &self.volume_id
    }

    #[must_use]
    pub fn root_id(&self) -> &LinkId {
        &self.root_id
    }

    #[must_use]
    pub fn address_id(&self) -> &AddressId {
        &self.address_id
    }

    #[must_use]
    pub fn key(&self) -> &PrivateKey {
        &self.key
    }

    #[must_use]
    pub fn session_key(&self) -> &SessionKey {
        &self.session_key
    }

    #[must_use]
    pub fn signature(&self) -> &SignatureStatus {
        &self.signature
    }

    pub(crate) fn new(
        id: ShareId,
        volume_id: VolumeId,
        root_id: LinkId,
        address_id: AddressId,
        unlocked: crate::crypto::DecryptedKey,
    ) -> ShareKeys {
        ShareKeys {
            id,
            volume_id,
            root_id,
            address_id,
            key: unlocked.key,
            session_key: unlocked.session_key,
            signature: unlocked.signature,
        }
    }
}

impl fmt::Debug for ShareKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ShareKeys")
            .field("id", &self.id)
            .field("volume_id", &self.volume_id)
            .field("root_id", &self.root_id)
            .field("address_id", &self.address_id)
            .field("key", &self.key.fingerprint())
            .field("signature", &self.signature)
            .finish_non_exhaustive()
    }
}
