use aes::Aes256;
use aes::cipher::block_padding::NoPadding;
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use hmac::{Hmac, KeyInit, Mac as _};
use p256::elliptic_curve::sec1::ToSec1Point;
use p256::{PublicKey, SecretKey};
use rikka_fido::cbor::Value;
use sha2::{Digest as _, Sha256};

pub(crate) fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("the system has randomness");
    bytes
}

pub(crate) fn sha256(data: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(data.as_ref()).into()
}

pub(crate) fn hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

pub(crate) fn agreement_key() -> SecretKey {
    loop {
        if let Ok(key) = SecretKey::from_slice(&random::<32>()) {
            return key;
        }
    }
}

pub(crate) fn cose(key: &SecretKey) -> Value {
    let point = key.public_key().to_sec1_point(false);
    Value::map([
        (Value::from(1), Value::from(2)),
        (Value::from(3), Value::from(-25)),
        (Value::from(-1), Value::from(1)),
        (
            Value::from(-2),
            Value::bytes(point.x().expect("affine").to_vec()),
        ),
        (
            Value::from(-3),
            Value::bytes(point.y().expect("affine").to_vec()),
        ),
    ])
}

pub(crate) fn shared_secret(protocol: u64, key: &SecretKey, peer: &Value) -> Option<Vec<u8>> {
    let x = peer.get(-2)?.as_bytes()?;
    let y = peer.get(-3)?.as_bytes()?;
    let point = PublicKey::from_sec1_bytes(&[&[0x04], x, y].concat()).ok()?;
    let shared = key.diffie_hellman(&point);
    let z = shared.raw_secret_bytes().to_vec();

    Some(match protocol {
        1 => sha256(&z).to_vec(),
        2 => {
            let extract = hmac(&[0u8; 32], &z);
            let hmac_key = hmac(&extract, b"CTAP2 HMAC key\x01");
            let aes_key = hmac(&extract, b"CTAP2 AES key\x01");
            [hmac_key.as_slice(), &aes_key].concat()
        }
        _ => return None,
    })
}

pub(crate) fn decrypt(protocol: u64, secret: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    let (iv, key, body) = match protocol {
        1 => ([0u8; 16], secret, sealed),
        _ => {
            let (iv, body) = sealed.split_at_checked(16)?;
            (iv.try_into().ok()?, &secret[32..], body)
        }
    };
    let mut plaintext = body.to_vec();
    let length = cbc::Decryptor::<Aes256>::new_from_slices(key, &iv)
        .ok()?
        .decrypt_padded::<NoPadding>(&mut plaintext)
        .ok()?
        .len();
    plaintext.truncate(length);
    Some(plaintext)
}

pub(crate) fn encrypt(protocol: u64, secret: &[u8], plain: &[u8]) -> Vec<u8> {
    let (iv, key) = match protocol {
        1 => ([0u8; 16], secret),
        _ => (random::<16>(), &secret[32..]),
    };
    let mut sealed = plain.to_vec();
    cbc::Encryptor::<Aes256>::new_from_slices(key, &iv)
        .expect("a 32-byte key and 16-byte iv")
        .encrypt_padded::<NoPadding>(&mut sealed, plain.len())
        .expect("whole blocks");
    match protocol {
        1 => sealed,
        _ => [iv.as_slice(), &sealed].concat(),
    }
}

pub(crate) fn verify(protocol: u64, key: &[u8], message: &[u8], param: &[u8]) -> bool {
    match protocol {
        1 => param == &hmac(key, message)[..16],
        _ => param == hmac(&key[..key.len().min(32)], message).as_slice(),
    }
}
