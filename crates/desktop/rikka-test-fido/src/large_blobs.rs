use std::io::Write as _;

use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit, Nonce, Payload};
use flate2::Compression;
use flate2::write::DeflateEncoder;
use rikka_fido::cbor::Value;

use crate::crypto;

pub(crate) fn serialize(blobs: &[([u8; 32], &[u8])]) -> Vec<u8> {
    let entries = blobs
        .iter()
        .map(|(key, blob)| {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(blob).expect("compressing in memory");
            let compressed = encoder.finish().expect("compressing in memory");

            let nonce = crypto::random::<12>();
            let size = blob.len() as u64;
            let aad = [b"blob".as_slice(), &size.to_le_bytes()].concat();
            let ciphertext = Aes256Gcm::new_from_slice(key)
                .expect("a 32-byte key")
                .encrypt(
                    &Nonce::<Aes256Gcm>::try_from(nonce.as_slice()).expect("a 12-byte nonce"),
                    Payload {
                        msg: &compressed,
                        aad: &aad,
                    },
                )
                .expect("sealing");

            Value::map([
                (Value::from(0x01), Value::bytes(ciphertext)),
                (Value::from(0x02), Value::bytes(nonce)),
                (Value::from(0x03), Value::from(size)),
            ])
        })
        .collect::<Vec<_>>();

    let mut serialized = Value::Array(entries).encode();
    let digest = crypto::sha256(&serialized);
    serialized.extend_from_slice(&digest[..16]);
    serialized
}
