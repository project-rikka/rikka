use std::fmt;

use rikka_proton_pgp::{Hash, SecretBytes, hmac};

#[derive(Clone)]
pub struct HashKey(SecretBytes);

impl HashKey {
    #[must_use]
    pub fn hash(&self, name: &str) -> String {
        rikka_types::hex::encode(hmac(Hash::Sha256, &self.0, name))
    }

    pub(crate) fn new(bytes: Vec<u8>) -> HashKey {
        HashKey(SecretBytes::new(bytes))
    }
}

impl fmt::Debug for HashKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HashKey")
            .field("bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}
