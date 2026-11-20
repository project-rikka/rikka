use rcgen::{KeyPair, PKCS_ECDSA_P256_SHA256, PublicKeyData as _, SigningKey as _};
use rikka_certificate_transparency::{LogList, LogListKey};

#[derive(Debug)]
pub struct Signer {
    key: KeyPair,
}

impl Signer {
    #[must_use]
    pub fn new() -> Signer {
        Signer {
            key: KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
                .expect("generating a P-256 key should succeed"),
        }
    }

    #[must_use]
    pub fn key(&self) -> LogListKey {
        LogListKey::from_spki_der(&self.key.subject_public_key_info())
            .expect("a P-256 key should be a log list key")
    }

    #[must_use]
    pub fn sign(&self, list: &[u8]) -> Vec<u8> {
        self.key.sign(list).expect("signing should succeed")
    }

    #[must_use]
    pub fn signed(&self, list: &[u8]) -> LogList {
        LogList::from_signed(list, &self.sign(list), &self.key())
            .expect("a freshly signed list should verify")
    }
}

impl Default for Signer {
    fn default() -> Signer {
        Signer::new()
    }
}
