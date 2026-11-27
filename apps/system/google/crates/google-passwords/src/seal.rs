use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chacha20poly1305::aead::AeadInOut;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use google_account::{SEAL, SavedAccount};
use rand::RngCore;
use rikka_file_format::{BinaryFormat, Reader};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::{Error, Result};

const LABEL: &str = "Google passwords on this device";
const FORMAT: BinaryFormat = BinaryFormat::new("rikka-google-passwords", *b"RKGPASSW");
const KEY: usize = 32;
const NONCE: usize = 24;

#[derive(Clone, PartialEq, Eq)]
pub struct Seal {
    key: Zeroizing<[u8; KEY]>,
}

#[derive(Serialize, Deserialize)]
struct Kept {
    key: String,
}

impl Seal {
    #[must_use]
    pub fn generate() -> Seal {
        let mut key = Zeroizing::new([0; KEY]);
        rand::rng().fill_bytes(key.as_mut_slice());
        Seal { key }
    }

    pub async fn kept(folder: &Path) -> Result<Seal> {
        let secret = SavedAccount::secret(folder, SEAL);
        if let Some(seal) = secret.kept().await? {
            return Ok(seal);
        }
        let seal = Seal::generate();
        secret.keep(LABEL, &seal).await?;
        Ok(seal)
    }
    pub(crate) fn seal(&self, plain: &[u8]) -> Vec<u8> {
        let header = FORMAT.header();
        let mut nonce = [0; NONCE];
        rand::rng().fill_bytes(&mut nonce);
        let mut body = Zeroizing::new(plain.to_vec());
        XChaCha20Poly1305::new(&(*self.key).into())
            .encrypt_in_place(&XNonce::from(nonce), &header, &mut *body)
            .expect("the snapshot fits one message");
        let mut sealed = Vec::with_capacity(header.len() + NONCE + body.len());
        sealed.extend_from_slice(&header);
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(&body);
        sealed
    }

    pub(crate) fn open(&self, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let mut reader = Reader::new(sealed);
        FORMAT.read_header(&mut reader)?;
        let nonce: [u8; NONCE] = reader.array()?;
        let mut body = Zeroizing::new(reader.rest().to_vec());
        XChaCha20Poly1305::new(&(*self.key).into())
            .decrypt_in_place(&XNonce::from(nonce), &FORMAT.header(), &mut *body)
            .map_err(|_| Error::Sealed)?;
        Ok(body)
    }
}

impl Serialize for Seal {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Kept {
            key: STANDARD.encode(self.key.as_slice()),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Seal {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Seal, D::Error> {
        let kept = Kept::deserialize(deserializer)?;
        let key: [u8; KEY] = STANDARD
            .decode(kept.key)
            .ok()
            .and_then(|key| key.try_into().ok())
            .ok_or_else(|| serde::de::Error::custom("the seal is not a 256-bit key"))?;
        Ok(Seal {
            key: Zeroizing::new(key),
        })
    }
}

impl std::fmt::Debug for Seal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Seal").finish_non_exhaustive()
    }
}
