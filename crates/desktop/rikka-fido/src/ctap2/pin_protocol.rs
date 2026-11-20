use aes::Aes256;
use aes::cipher::block_padding::NoPadding;
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use p256::elliptic_curve::sec1::ToSec1Point;
use p256::{PublicKey, SecretKey};
use zeroize::Zeroizing;

use crate::cbor::Value;
use crate::crypto;
use crate::error::{Error, Result};

const BLOCK: usize = 16;
const KEY_TYPE_EC2: i64 = 2;
const CURVE_P256: i64 = 1;
const ECDH_ES_HKDF_256: i64 = -25;
const HMAC_INFO: &[u8] = b"CTAP2 HMAC key";
const AES_INFO: &[u8] = b"CTAP2 AES key";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PinProtocol {
    One,
    Two,
}

impl PinProtocol {
    pub(crate) fn from_number(number: u64) -> Option<PinProtocol> {
        match number {
            1 => Some(PinProtocol::One),
            2 => Some(PinProtocol::Two),
            _ => None,
        }
    }

    pub(crate) fn number(self) -> u64 {
        match self {
            PinProtocol::One => 1,
            PinProtocol::Two => 2,
        }
    }

    pub(crate) fn encapsulate(self, peer: &Value) -> Result<(Value, SharedSecret)> {
        let peer = peer_key(peer)?;
        let secret = loop {
            let candidate = Zeroizing::new(crypto::random::<32>()?);
            if let Ok(secret) = SecretKey::from_slice(candidate.as_slice()) {
                break secret;
            }
        };

        let point = secret.public_key().to_sec1_point(false);
        let (Some(x), Some(y)) = (point.x(), point.y()) else {
            return Err(Error::Protocol("platform key has no affine coordinates"));
        };
        let platform = Value::map([
            (Value::from(1), Value::from(KEY_TYPE_EC2)),
            (Value::from(3), Value::from(ECDH_ES_HKDF_256)),
            (Value::from(-1), Value::from(CURVE_P256)),
            (Value::from(-2), Value::bytes(x.to_vec())),
            (Value::from(-3), Value::bytes(y.to_vec())),
        ]);

        let shared = secret.diffie_hellman(&peer);
        let z = shared.raw_secret_bytes();
        let key = match self {
            PinProtocol::One => Zeroizing::new(crypto::sha256(z).to_vec()),
            PinProtocol::Two => {
                let mut key = Zeroizing::new(Vec::with_capacity(64));
                key.extend_from_slice(&hkdf(z, HMAC_INFO));
                key.extend_from_slice(&hkdf(z, AES_INFO));
                key
            }
        };

        Ok((
            platform,
            SharedSecret {
                protocol: self,
                key,
            },
        ))
    }

    pub(crate) fn authenticate(self, key: &[u8], message: &[u8]) -> Vec<u8> {
        match self {
            PinProtocol::One => crypto::hmac_sha256(key, &[message])[..16].to_vec(),
            PinProtocol::Two => crypto::hmac_sha256(&key[..key.len().min(32)], &[message]).to_vec(),
        }
    }
}

pub(crate) struct SharedSecret {
    protocol: PinProtocol,
    key: Zeroizing<Vec<u8>>,
}

impl SharedSecret {
    pub(crate) fn protocol(&self) -> PinProtocol {
        self.protocol
    }

    pub(crate) fn authenticate(&self, message: &[u8]) -> Vec<u8> {
        self.protocol.authenticate(&self.key, message)
    }
    pub(crate) fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>> {
        if plain.is_empty() || !plain.len().is_multiple_of(BLOCK) {
            return Err(Error::Protocol("PIN protocol plaintext not block-aligned"));
        }

        let (iv, key) = match self.protocol {
            PinProtocol::One => ([0u8; BLOCK], &self.key[..]),
            PinProtocol::Two => (crypto::random::<BLOCK>()?, &self.key[32..]),
        };

        let mut sealed = plain.to_vec();
        cbc::Encryptor::<Aes256>::new_from_slices(key, &iv)
            .map_err(|_| Error::Protocol("invalid PIN protocol key length"))?
            .encrypt_padded::<NoPadding>(&mut sealed, plain.len())
            .map_err(|_| Error::Protocol("PIN protocol plaintext not block-aligned"))?;

        Ok(match self.protocol {
            PinProtocol::One => sealed,
            PinProtocol::Two => [iv.as_slice(), &sealed].concat(),
        })
    }

    pub(crate) fn decrypt(&self, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let (iv, key, body) = match self.protocol {
            PinProtocol::One => ([0u8; BLOCK], &self.key[..], sealed),
            PinProtocol::Two => {
                if sealed.len() < BLOCK {
                    return Err(Error::Protocol("encrypted PIN token missing IV"));
                }
                let (iv, body) = sealed.split_at(BLOCK);
                let iv: [u8; BLOCK] = iv.try_into().expect("IV is one block");
                (iv, &self.key[32..], body)
            }
        };

        if body.is_empty() || !body.len().is_multiple_of(BLOCK) {
            return Err(Error::Protocol("encrypted PIN token not block-aligned"));
        }

        let mut plaintext = Zeroizing::new(body.to_vec());
        let length = cbc::Decryptor::<Aes256>::new_from_slices(key, &iv)
            .map_err(|_| Error::Protocol("invalid PIN protocol key length"))?
            .decrypt_padded::<NoPadding>(&mut plaintext)
            .map_err(|_| Error::Protocol("encrypted PIN token not block-aligned"))?
            .len();
        plaintext.truncate(length);
        Ok(plaintext)
    }
}

fn peer_key(cose: &Value) -> Result<PublicKey> {
    let malformed = Error::Protocol("key agreement key is not a P-256 COSE key");

    if cose.get(1).and_then(Value::as_i64) != Some(KEY_TYPE_EC2)
        || cose.get(-1).and_then(Value::as_i64) != Some(CURVE_P256)
    {
        return Err(malformed);
    }

    let (Some(x), Some(y)) = (
        cose.get(-2).and_then(Value::as_bytes),
        cose.get(-3).and_then(Value::as_bytes),
    ) else {
        return Err(malformed);
    };
    if x.len() != 32 || y.len() != 32 {
        return Err(malformed);
    }

    let encoded = [&[0x04], x, y].concat();
    PublicKey::from_sec1_bytes(&encoded).map_err(|_| malformed)
}

fn hkdf(ikm: &[u8], info: &[u8]) -> [u8; 32] {
    let pseudo_random = Zeroizing::new(crypto::hmac_sha256(&[0u8; 32], &[ikm]));
    crypto::hmac_sha256(pseudo_random.as_slice(), &[info, &[0x01]])
}
