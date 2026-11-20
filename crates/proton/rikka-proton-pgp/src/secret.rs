use std::fmt;

use base64::prelude::{BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::random::fill_random;

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Secret {
        Secret(value.into())
    }

    #[must_use]
    pub fn random_hex(length: usize) -> Secret {
        Secret(rikka_types::hex::encode(
            SecretBytes::random(length).expose(),
        ))
    }

    #[must_use]
    pub fn random_base64(length: usize) -> Secret {
        let bytes = SecretBytes::random(length);
        let mut output = String::with_capacity(length.div_ceil(3) * 4);
        BASE64_STANDARD.encode_string(bytes.expose(), &mut output);
        Secret(output)
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Secret {
        Secret(value)
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Secret {
        Secret(value.to_owned())
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_redacted(f, self.0.len())
    }
}

#[derive(Clone, Default, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    pub fn new(bytes: impl Into<Vec<u8>>) -> SecretBytes {
        SecretBytes(bytes.into())
    }

    #[must_use]
    pub fn random(length: usize) -> SecretBytes {
        let mut bytes = SecretBytes(vec![0; length]);
        fill_random(&mut bytes.0);
        bytes
    }

    #[must_use]
    pub fn random_hex(length: usize) -> SecretBytes {
        let text = Secret::random_hex(length);
        SecretBytes(text.expose().as_bytes().to_vec())
    }

    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl AsRef<[u8]> for SecretBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for SecretBytes {
    fn from(bytes: Vec<u8>) -> SecretBytes {
        SecretBytes(bytes)
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_redacted(f, self.0.len())
    }
}

fn write_redacted(f: &mut fmt::Formatter<'_>, length: usize) -> fmt::Result {
    match length {
        0 => f.write_str("\"\""),
        length => write!(f, "\"<{length} hidden bytes>\""),
    }
}
