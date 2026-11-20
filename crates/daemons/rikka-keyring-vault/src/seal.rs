use chacha20poly1305::aead::AeadInOut;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::locked::KEY_BYTES;

pub(crate) const NONCE_BYTES: usize = 24;

const TAG_BYTES: usize = 16;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Sealed {
    pub(crate) nonce: [u8; NONCE_BYTES],
    pub(crate) ciphertext: Vec<u8>,
}

impl Sealed {
    pub(crate) fn seal(key: &[u8; KEY_BYTES], aad: &[u8], plain: &[u8]) -> Result<Sealed> {
        let mut nonce = [0u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(Error::Random)?;
        let mut buffer = Zeroizing::new(Vec::with_capacity(plain.len() + TAG_BYTES));
        buffer.extend_from_slice(plain);
        cipher(key)?
            .encrypt_in_place(&XNonce::from(nonce), aad, &mut *buffer)
            .map_err(|_| Error::Cipher)?;
        Ok(Sealed {
            nonce,
            ciphertext: buffer.to_vec(),
        })
    }

    pub(crate) fn open(&self, key: &[u8; KEY_BYTES], aad: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
        let mut buffer = Zeroizing::new(self.ciphertext.clone());
        cipher(key)
            .ok()?
            .decrypt_in_place(&XNonce::from(self.nonce), aad, &mut *buffer)
            .ok()?;
        Some(buffer)
    }
}

impl std::fmt::Debug for Sealed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Sealed")
            .field("len", &self.ciphertext.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn framed(parts: &[&[u8]]) -> Vec<u8> {
    let mut framed = Vec::with_capacity(parts.iter().map(|part| part.len() + 8).sum());
    for part in parts {
        framed.extend_from_slice(&(part.len() as u64).to_be_bytes());
        framed.extend_from_slice(part);
    }
    framed
}

fn cipher(key: &[u8; KEY_BYTES]) -> Result<XChaCha20Poly1305> {
    XChaCha20Poly1305::new_from_slice(key).map_err(|_| Error::Cipher)
}
