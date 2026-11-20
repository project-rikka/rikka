use std::io::Read as _;

use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit, Nonce, Payload};
use flate2::read::DeflateDecoder;

use super::command::{self, LARGE_BLOBS};
use super::info::Info;
use crate::cbor::Value;
use crate::ceremony::Watch;
use crate::crypto;
use crate::ctaphid::Channel;
use crate::error::{Error, Result};

pub(crate) const LARGE_BLOB_KEY: &str = "largeBlobKey";

const MESSAGE_OVERHEAD: u64 = 64;
const DEFAULT_MESSAGE: u64 = 1024;
const DIGEST: usize = 16;
const NONCE: usize = 12;
const AAD_PREFIX: &[u8] = b"blob";

pub(crate) fn supported(info: &Info) -> bool {
    info.extensions().iter().any(|name| name == LARGE_BLOB_KEY)
        && info.option("largeBlobs") == Some(true)
}

pub(crate) fn read(
    channel: &mut Channel,
    info: &Info,
    key: &[u8],
    watch: &Watch,
) -> Result<Option<Vec<u8>>> {
    let array = read_array(channel, info, watch)?;
    if array.len() <= DIGEST {
        return Ok(None);
    }

    let (body, digest) = array.split_at(array.len() - DIGEST);
    if crypto::sha256(body)[..DIGEST] != *digest {
        return Ok(None);
    }

    let entries = Value::decode(body)?;
    let entries = entries
        .as_array()
        .ok_or(Error::Protocol("large-blob array is not an array"))?;
    Ok(entries.iter().find_map(|entry| open(entry, key)))
}

fn read_array(channel: &mut Channel, info: &Info, watch: &Watch) -> Result<Vec<u8>> {
    let chunk = info
        .max_message_size()
        .unwrap_or(DEFAULT_MESSAGE)
        .saturating_sub(MESSAGE_OVERHEAD)
        .max(1);
    let limit = info.max_large_blob_array().unwrap_or(u64::from(u32::MAX));

    let mut array = Vec::new();
    loop {
        let request = Value::map([
            (Value::from(0x01), Value::from(chunk)),
            (Value::from(0x03), Value::from(array.len())),
        ]);
        let reply = command::call(channel, LARGE_BLOBS, Some(&request), watch)?;
        let fragment = reply
            .get(0x01)
            .and_then(Value::as_bytes)
            .ok_or(Error::Protocol("large-blob response missing fragment"))?;
        array.extend_from_slice(fragment);

        if array.len() as u64 > limit {
            return Err(Error::Protocol("large-blob array exceeds advertised size"));
        }
        if (fragment.len() as u64) < chunk {
            return Ok(array);
        }
    }
}

fn open(entry: &Value, key: &[u8]) -> Option<Vec<u8>> {
    let ciphertext = entry.get(0x01)?.as_bytes()?;
    let nonce = entry.get(0x02)?.as_bytes()?;
    let original = entry.get(0x03)?.as_unsigned()?;
    if nonce.len() != NONCE {
        return None;
    }

    let aad = [AAD_PREFIX, &original.to_le_bytes()].concat();
    let nonce = Nonce::<Aes256Gcm>::try_from(nonce).ok()?;
    let compressed = Aes256Gcm::new_from_slice(key)
        .ok()?
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .ok()?;

    let mut blob = Vec::new();
    DeflateDecoder::new(compressed.as_slice())
        .take(original.saturating_add(1))
        .read_to_end(&mut blob)
        .ok()?;
    (blob.len() as u64 == original).then_some(blob)
}
