use chacha20poly1305::aead::AeadInOut;
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::header::MAC_BYTES;

pub(crate) const FILE_KEY_BYTES: usize = 16;
pub(crate) const KEY_BYTES: usize = 32;
pub(crate) const PAYLOAD_NONCE_BYTES: usize = 16;

const HEADER_INFO: &[u8] = b"header";
const PAYLOAD_INFO: &[u8] = b"payload";

pub(crate) type FileKey = Zeroizing<[u8; FILE_KEY_BYTES]>;
pub(crate) type SymmetricKey = Zeroizing<[u8; KEY_BYTES]>;

pub(crate) fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(Error::Random)?;
    Ok(bytes)
}

pub(crate) fn header_mac(file_key: &FileKey, authenticated: &[u8]) -> [u8; MAC_BYTES] {
    let key = derive(&[], file_key.as_slice(), HEADER_INFO);
    let mut mac = header_hmac(&key);
    mac.update(authenticated);
    mac.finalize().into_bytes().into()
}

pub(crate) fn verify_header(
    file_key: &FileKey,
    authenticated: &[u8],
    expected: &[u8; MAC_BYTES],
) -> Result<()> {
    let key = derive(&[], file_key.as_slice(), HEADER_INFO);
    let mut mac = header_hmac(&key);
    mac.update(authenticated);
    mac.verify_slice(expected).map_err(|_| Error::Tampered)
}

pub(crate) fn payload_key(file_key: &FileKey, nonce: &[u8; PAYLOAD_NONCE_BYTES]) -> SymmetricKey {
    derive(nonce, file_key.as_slice(), PAYLOAD_INFO)
}

pub(crate) fn cipher(key: &SymmetricKey) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(&Key::from(**key))
}

pub(crate) fn seal(key: &SymmetricKey, nonce: [u8; 12], buffer: &mut Vec<u8>) -> Result<()> {
    cipher(key)
        .encrypt_in_place(&Nonce::from(nonce), &[], buffer)
        .map_err(|_| Error::Payload)
}

pub(crate) fn open(key: &SymmetricKey, nonce: [u8; 12], buffer: &mut Vec<u8>) -> bool {
    cipher(key)
        .decrypt_in_place(&Nonce::from(nonce), &[], buffer)
        .is_ok()
}

fn derive(salt: &[u8], ikm: &[u8], info: &[u8]) -> SymmetricKey {
    let mut key = Zeroizing::new([0u8; KEY_BYTES]);
    Hkdf::<Sha256>::new(Some(salt), ikm)
        .expand(info, key.as_mut_slice())
        .expect("HKDF-SHA-256 expands to 32 bytes");
    key
}

fn header_hmac(key: &SymmetricKey) -> Hmac<Sha256> {
    <Hmac<Sha256> as KeyInit>::new_from_slice(key.as_slice()).expect("HMAC accepts any key length")
}
