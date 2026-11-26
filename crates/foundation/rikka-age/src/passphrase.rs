use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::header::{Stanza, decode};
use crate::keys::{self, FILE_KEY_BYTES, FileKey, KEY_BYTES, SymmetricKey};

pub(crate) const TAG: &str = "scrypt";

const LABEL: &[u8] = b"age-encryption.org/v1/scrypt";
const SALT_BYTES: usize = 16;
const BLOCK_SIZE: u32 = 8;
const PARALLELISM: u32 = 1;
const ZERO_NONCE: [u8; 12] = [0; 12];

pub(crate) fn wrap(passphrase: &str, work_factor: u8, file_key: &FileKey) -> Result<Stanza> {
    let salt: [u8; SALT_BYTES] = keys::random()?;
    let key = derive(passphrase, &salt, work_factor)?;
    let mut body = file_key.to_vec();
    keys::seal(&key, ZERO_NONCE, &mut body)?;
    Ok(Stanza {
        tag: TAG.to_owned(),
        args: vec![STANDARD_NO_PAD.encode(salt), work_factor.to_string()],
        body,
    })
}

pub(crate) fn work_factor(stanza: &Stanza) -> Result<u8> {
    let [_, factor] = stanza.args.as_slice() else {
        return Err(Error::Header(
            "scrypt stanza needs a salt and a work factor",
        ));
    };
    if factor.starts_with('0') || !factor.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::Header("malformed scrypt work factor"));
    }
    match factor.parse::<u8>() {
        Ok(factor) if factor > 0 => Ok(factor),
        _ => Err(Error::Header("malformed scrypt work factor")),
    }
}

pub(crate) fn unwrap(passphrase: &str, stanza: &Stanza, allowed: u8) -> Result<FileKey> {
    let factor = work_factor(stanza)?;
    if factor > allowed {
        return Err(Error::WorkFactor { factor, allowed });
    }
    let salt: [u8; SALT_BYTES] = decode(stanza.args[0].as_bytes())?
        .try_into()
        .map_err(|_| Error::Header("scrypt salt must be 16 bytes"))?;
    if stanza.body.len() != FILE_KEY_BYTES + 16 {
        return Err(Error::Header("scrypt body must be 32 bytes"));
    }
    let key = derive(passphrase, &salt, factor)?;
    let mut body = Zeroizing::new(stanza.body.clone());
    if !keys::open(&key, ZERO_NONCE, &mut body) {
        return Err(Error::Passphrase);
    }
    let mut file_key = Zeroizing::new([0u8; FILE_KEY_BYTES]);
    file_key.copy_from_slice(&body);
    Ok(file_key)
}

fn derive(passphrase: &str, salt: &[u8; SALT_BYTES], work_factor: u8) -> Result<SymmetricKey> {
    let params = scrypt::Params::new(work_factor, BLOCK_SIZE, PARALLELISM)
        .map_err(|_| Error::Header("unsupported scrypt work factor"))?;
    let mut labelled = Vec::with_capacity(LABEL.len() + SALT_BYTES);
    labelled.extend_from_slice(LABEL);
    labelled.extend_from_slice(salt);
    let mut key = Zeroizing::new([0u8; KEY_BYTES]);
    scrypt::scrypt(
        passphrase.as_bytes(),
        &labelled,
        &params,
        key.as_mut_slice(),
    )
    .expect("scrypt derives 32 bytes");
    Ok(key)
}
