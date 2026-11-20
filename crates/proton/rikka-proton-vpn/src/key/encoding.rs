use std::fmt;

use crate::error::{Error, Result};

pub(crate) const ED25519_SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2A, 0x30, 0x05, 0x06, 0x03, 0x2B, 0x65, 0x70, 0x03, 0x21, 0x00,
];

pub(crate) const ED25519_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2E, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2B, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

pub(crate) const KEY_LENGTH: usize = 32;

const PEM_LINE_LEN: usize = 64;

pub(crate) fn encode(bytes: &[u8]) -> String {
    rikka_proton_api::base64::encode(bytes)
}

pub(crate) fn decode(subject: &'static str, text: &str) -> Result<Vec<u8>> {
    rikka_proton_api::base64::decode(text.trim()).map_err(|_| Error::NotBase64 { subject })
}

pub(crate) fn decode_key(subject: &'static str, text: &str) -> Result<[u8; KEY_LENGTH]> {
    let bytes = decode(subject, text)?;

    <[u8; KEY_LENGTH]>::try_from(bytes.as_slice()).map_err(|_| Error::KeyLength {
        subject,
        length: bytes.len(),
    })
}

pub(crate) fn pem(label: &str, bytes: &[u8]) -> String {
    Pem {
        label,
        body: encode(bytes),
    }
    .to_string()
}

struct Pem<'a> {
    label: &'a str,
    body: String,
}

impl fmt::Display for Pem<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = self.label;
        writeln!(out, "-----BEGIN {label}-----")?;
        let mut rest = self.body.as_str();
        while !rest.is_empty() {
            let (line, remaining) = rest.split_at(PEM_LINE_LEN.min(rest.len()));
            writeln!(out, "{line}")?;
            rest = remaining;
        }
        writeln!(out, "-----END {label}-----")
    }
}
