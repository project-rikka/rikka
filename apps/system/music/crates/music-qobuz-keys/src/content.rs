use aes::Aes128;
use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockModeDecrypt, KeyIvInit, StreamCipher};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hkdf::Hkdf;
use music_core::{Error, Result};
use sha2::Sha256;

const WRAPPING: &str = "qbz-1";

pub type ContentKey = [u8; 16];

pub fn content_key(secret: &str, infos: &str, wrapped: &str) -> Result<ContentKey> {
    unwrap(&session_key(secret, infos)?, wrapped)
}

pub fn decrypt_frame(key: &ContentKey, iv: &[u8; 8], frame: &mut [u8]) {
    let mut nonce = [0; 16];
    nonce[..8].copy_from_slice(iv);
    ctr::Ctr128BE::<Aes128>::new(&(*key).into(), &nonce.into()).apply_keystream(frame);
}

fn session_key(secret: &str, infos: &str) -> Result<ContentKey> {
    let (salt, info) = infos
        .split_once('.')
        .ok_or_else(|| unreadable("the session has no key salt"))?;
    let seed = rikka_types::hex::decode(secret)
        .map_err(|_| unreadable("the app secret is not hexadecimal"))?;
    let salt = URL_SAFE_NO_PAD
        .decode(salt)
        .map_err(|_| unreadable("the session key salt is not base64"))?;
    let info = URL_SAFE_NO_PAD
        .decode(info.split('.').next().unwrap_or_default())
        .map_err(|_| unreadable("the session key info is not base64"))?;
    let mut key = [0; 16];
    Hkdf::<Sha256>::new(Some(&salt), &seed)
        .expand(&info, &mut key)
        .map_err(|_| unreadable("the key cannot be derived"))?;
    Ok(key)
}

fn unwrap(session: &ContentKey, wrapped: &str) -> Result<ContentKey> {
    let mut parts = wrapped.split('.');
    if parts.next() != Some(WRAPPING) {
        return Err(unreadable("the track key is wrapped in an unknown way"));
    }
    let (Some(sealed), Some(iv)) = (parts.next(), parts.next()) else {
        return Err(unreadable("the track key is incomplete"));
    };
    let mut sealed = URL_SAFE_NO_PAD
        .decode(sealed)
        .map_err(|_| unreadable("the track key is not base64"))?;
    let iv: [u8; 16] = URL_SAFE_NO_PAD
        .decode(iv)
        .ok()
        .and_then(|iv| iv.try_into().ok())
        .ok_or_else(|| unreadable("the track key has no usable iv"))?;
    let decrypted = cbc::Decryptor::<Aes128>::new(&(*session).into(), &iv.into())
        .decrypt_padded::<Pkcs7>(&mut sealed)
        .map_err(|_| unreadable("the track key does not open with this session"))?;
    decrypted
        .try_into()
        .map_err(|_| unreadable("the track key is not 16 bytes"))
}

pub(crate) fn unreadable(message: &str) -> Error {
    Error::Unreadable(message.to_owned())
}
