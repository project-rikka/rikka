use std::fmt;

use serde::de::{Deserialize, Deserializer, Error as _};
use serde::ser::{Serialize, Serializer};
use x25519_dalek::{X25519_BASEPOINT_BYTES, x25519};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::Result;
use crate::key::encoding::{self, KEY_LENGTH};

#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct X25519SecretKey([u8; KEY_LENGTH]);

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct X25519PublicKey([u8; KEY_LENGTH]);

impl X25519SecretKey {
    #[must_use]
    pub fn from_bytes(bytes: [u8; KEY_LENGTH]) -> X25519SecretKey {
        X25519SecretKey(bytes)
    }

    pub fn from_base64(text: &str) -> Result<X25519SecretKey> {
        Ok(X25519SecretKey(encoding::decode_key(
            "X25519 secret key",
            text,
        )?))
    }

    #[must_use]
    pub fn to_bytes(&self) -> [u8; KEY_LENGTH] {
        self.0
    }

    #[must_use]
    pub fn to_base64(&self) -> String {
        encoding::encode(&self.0)
    }

    #[must_use]
    pub fn public_key(&self) -> X25519PublicKey {
        X25519PublicKey(x25519(self.0, X25519_BASEPOINT_BYTES))
    }
}

impl X25519PublicKey {
    #[must_use]
    pub fn from_bytes(bytes: [u8; KEY_LENGTH]) -> X25519PublicKey {
        X25519PublicKey(bytes)
    }

    pub fn from_base64(text: &str) -> Result<X25519PublicKey> {
        Ok(X25519PublicKey(encoding::decode_key(
            "X25519 public key",
            text,
        )?))
    }

    #[must_use]
    pub fn to_bytes(&self) -> [u8; KEY_LENGTH] {
        self.0
    }

    #[must_use]
    pub fn to_base64(&self) -> String {
        encoding::encode(&self.0)
    }

    #[must_use]
    pub fn fingerprint(&self) -> String {
        encoding::encode(&rikka_proton_pgp::sha512(self.0))
    }
}

impl fmt::Debug for X25519SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("X25519SecretKey(\"<32 hidden bytes>\")")
    }
}

impl fmt::Debug for X25519PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "X25519PublicKey({:?})", self.to_base64())
    }
}

impl fmt::Display for X25519PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_base64())
    }
}

impl Serialize for X25519PublicKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_base64())
    }
}

impl<'de> Deserialize<'de> for X25519PublicKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        X25519PublicKey::from_base64(&text).map_err(D::Error::custom)
    }
}
