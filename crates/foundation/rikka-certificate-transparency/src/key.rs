use rustls_pki_types::SubjectPublicKeyInfoDer;
use rustls_pki_types::pem::PemObject as _;

use crate::error::{Error, Result};
use crate::public_key::PublicKey;

const CHROME: &[u8] = include_bytes!("../data/log_list_pubkey.pem");

#[derive(Debug, Clone)]
pub struct LogListKey {
    key: PublicKey,
}

impl LogListKey {
    #[must_use]
    pub fn chrome() -> LogListKey {
        let der = SubjectPublicKeyInfoDer::from_pem_slice(CHROME)
            .expect("Chrome's log list key should be a PEM public key");
        LogListKey::from_spki_der(&der).expect("Chrome's log list key should be an RSA key")
    }

    pub fn from_spki_der(der: &[u8]) -> Result<LogListKey> {
        PublicKey::from_spki(der)
            .map(|key| LogListKey { key })
            .ok_or(Error::UnsupportedKey)
    }

    pub(crate) fn verifies(&self, list: &[u8], signature: &[u8]) -> bool {
        self.key.verifies(list, signature)
    }
}
