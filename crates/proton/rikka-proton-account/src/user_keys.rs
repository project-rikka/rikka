use std::fmt;

use rikka_proton_pgp::{KeyRing, PrivateKey};

use crate::account_keys::AccountKeys;

#[derive(Clone)]
pub struct UserKeys {
    ring: KeyRing,
    signer: PrivateKey,
}

impl UserKeys {
    #[must_use]
    pub fn new(ring: KeyRing, signer: PrivateKey) -> UserKeys {
        UserKeys { ring, signer }
    }

    #[must_use]
    pub fn with_address_keys(self, keys: &AccountKeys) -> UserKeys {
        let ring = keys.address_rings().fold(self.ring, |ring, address| {
            ring.with_private_keys(address.private_keys().to_vec())
        });
        UserKeys { ring, ..self }
    }

    #[must_use]
    pub fn ring(&self) -> &KeyRing {
        &self.ring
    }

    #[must_use]
    pub fn signer(&self) -> &PrivateKey {
        &self.signer
    }
}

impl fmt::Debug for UserKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UserKeys")
            .field("keys", &self.ring.len())
            .finish_non_exhaustive()
    }
}
