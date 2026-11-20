use std::fmt;

use aes_gcm::aead::consts::U16;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::aes::Aes256;
use aes_gcm::{Aes256Gcm, AesGcm};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::random::fill_random;

type Aes256GcmLongNonce = AesGcm<Aes256, U16>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum NonceLength {
    #[default]
    Standard,
    Long,
}

impl NonceLength {
    #[must_use]
    pub const fn bytes(self) -> usize {
        match self {
            NonceLength::Standard => 12,
            NonceLength::Long => 16,
        }
    }
}

#[derive(Clone)]
pub struct SymmetricKey {
    bytes: Zeroizing<[u8; SymmetricKey::LENGTH]>,
    nonce: NonceLength,
}

impl SymmetricKey {
    pub const LENGTH: usize = 32;
    pub const NONCE_LENGTH: usize = NonceLength::Standard.bytes();

    #[must_use]
    pub fn generate() -> SymmetricKey {
        let mut bytes = Zeroizing::new([0u8; SymmetricKey::LENGTH]);
        fill_random(bytes.as_mut_slice());

        SymmetricKey {
            bytes,
            nonce: NonceLength::Standard,
        }
    }

    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<SymmetricKey> {
        let key_bytes = <[u8; SymmetricKey::LENGTH]>::try_from(bytes.as_ref())
            .map_err(|_| Error::Invalid("symmetric key must be 32 bytes"))?;

        Ok(SymmetricKey {
            bytes: Zeroizing::new(key_bytes),
            nonce: NonceLength::Standard,
        })
    }

    #[must_use]
    pub fn with_nonce(mut self, nonce: NonceLength) -> SymmetricKey {
        self.nonce = nonce;
        self
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    pub fn seal(&self, data: impl AsRef<[u8]>, aad: impl AsRef<[u8]>) -> Result<Vec<u8>> {
        let mut nonce = [0u8; NonceLength::Long.bytes()];
        let nonce = &mut nonce[..self.nonce.bytes()];
        fill_random(nonce);

        let payload = Payload {
            msg: data.as_ref(),
            aad: aad.as_ref(),
        };
        let ciphertext = match self.nonce {
            NonceLength::Standard => encrypt::<Aes256Gcm>(self.as_bytes(), nonce, payload),
            NonceLength::Long => encrypt::<Aes256GcmLongNonce>(self.as_bytes(), nonce, payload),
        }?;

        let mut output = Vec::with_capacity(nonce.len() + ciphertext.len());
        output.extend_from_slice(nonce);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    pub fn open(&self, sealed: impl AsRef<[u8]>, aad: impl AsRef<[u8]>) -> Result<Vec<u8>> {
        let sealed = sealed.as_ref();
        if sealed.len() <= self.nonce.bytes() {
            return Err(Error::Invalid("sealed message is shorter than its nonce"));
        }

        let (nonce, msg) = sealed.split_at(self.nonce.bytes());
        let payload = Payload {
            msg,
            aad: aad.as_ref(),
        };
        match self.nonce {
            NonceLength::Standard => decrypt::<Aes256Gcm>(self.as_bytes(), nonce, payload),
            NonceLength::Long => decrypt::<Aes256GcmLongNonce>(self.as_bytes(), nonce, payload),
        }
    }
}

fn cipher<C: KeyInit>(key: &[u8]) -> Result<C> {
    C::new_from_slice(key).map_err(|_| Error::Invalid("symmetric key must be 32 bytes"))
}

fn encrypt<C: Aead + KeyInit>(
    key: &[u8],
    nonce: &[u8],
    payload: Payload<'_, '_>,
) -> Result<Vec<u8>> {
    let nonce = aes_gcm::aead::Nonce::<C>::try_from(nonce)
        .map_err(|_| Error::Invalid("invalid AES-GCM nonce length"))?;
    cipher::<C>(key)?
        .encrypt(&nonce, payload)
        .map_err(|error| Error::EncryptionFailed(Box::new(error)))
}

fn decrypt<C: Aead + KeyInit>(
    key: &[u8],
    nonce: &[u8],
    payload: Payload<'_, '_>,
) -> Result<Vec<u8>> {
    let nonce = aes_gcm::aead::Nonce::<C>::try_from(nonce)
        .map_err(|_| Error::Invalid("invalid AES-GCM nonce length"))?;
    cipher::<C>(key)?
        .decrypt(&nonce, payload)
        .map_err(|error| Error::DecryptionFailed(Box::new(error)))
}

impl fmt::Debug for SymmetricKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SymmetricKey")
            .field("nonce", &self.nonce)
            .finish_non_exhaustive()
    }
}
