use rikka_proton_pgp::{Hash, SecretBytes, SymmetricKey, hmac};

use crate::error::Result;

const SEALING: &[u8] = b"rikka-proton-cache/sealing";
const NAMING: &[u8] = b"rikka-proton-cache/naming";
const RECORD: &[u8] = b"record";
const BLOB: &[u8] = b"blob";

pub(crate) struct Keys {
    sealing: SymmetricKey,
    naming: SecretBytes,
}

impl Keys {
    pub(crate) fn derive(key: &SymmetricKey) -> Result<Keys> {
        let sealing = SecretBytes::new(hmac(Hash::Sha256, key.as_bytes(), SEALING));
        Ok(Keys {
            sealing: SymmetricKey::from_bytes(sealing.expose())?,
            naming: SecretBytes::new(hmac(Hash::Sha256, key.as_bytes(), NAMING)),
        })
    }

    pub(crate) fn record_key(&self, table: &str, id: &str) -> Vec<u8> {
        hmac(
            Hash::Sha256,
            self.naming.expose(),
            framed(&[RECORD, table.as_bytes(), id.as_bytes()]),
        )
    }

    pub(crate) fn blob_key(&self, area: &str, id: &str) -> Vec<u8> {
        hmac(
            Hash::Sha256,
            self.naming.expose(),
            framed(&[BLOB, area.as_bytes(), id.as_bytes()]),
        )
    }

    pub(crate) fn seal_record(&self, table: &str, key: &[u8], plain: &[u8]) -> Result<Vec<u8>> {
        Ok(self.sealing.seal(plain, record_identity(table, key))?)
    }

    pub(crate) fn open_record(&self, table: &str, key: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
        self.sealing.open(sealed, record_identity(table, key)).ok()
    }

    pub(crate) fn seal_blob(&self, area: &str, key: &[u8], plain: &[u8]) -> Result<Vec<u8>> {
        Ok(self
            .sealing
            .seal(plain, framed(&[BLOB, area.as_bytes(), key]))?)
    }

    pub(crate) fn open_blob(&self, area: &str, key: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
        self.sealing
            .open(sealed, framed(&[BLOB, area.as_bytes(), key]))
            .ok()
    }
}

fn record_identity(table: &str, key: &[u8]) -> Vec<u8> {
    framed(&[RECORD, table.as_bytes(), key])
}

fn framed(parts: &[&[u8]]) -> Vec<u8> {
    let mut framed = Vec::with_capacity(parts.iter().map(|part| part.len() + 8).sum());
    for part in parts {
        framed.extend_from_slice(&(part.len() as u64).to_be_bytes());
        framed.extend_from_slice(part);
    }
    framed
}
