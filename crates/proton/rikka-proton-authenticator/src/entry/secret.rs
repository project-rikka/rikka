use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;

use crate::base32::Base32;
use crate::error::{Error, Result};

fn normalize(secret: &str) -> String {
    secret
        .replace([' ', '-', '_'], "")
        .trim_end_matches('=')
        .to_ascii_uppercase()
}

pub(crate) fn decode(secret: &str) -> Result<Vec<u8>> {
    let normalized = normalize(secret);
    if normalized.is_empty() {
        return Err(Error::EmptySecret);
    }
    Base32::decode(&normalized).map_err(|_| Error::MalformedSecret)
}

pub(crate) fn decode_steam(secret: &str) -> Result<Vec<u8>> {
    let trimmed = secret.trim().trim_end_matches('=');
    if trimmed.is_empty() {
        return Err(Error::EmptySecret);
    }
    match Base32::decode(trimmed) {
        Ok(bytes) => Ok(bytes),
        Err(_) => STANDARD_NO_PAD
            .decode(trimmed)
            .map_err(|_| Error::MalformedSecret),
    }
}
