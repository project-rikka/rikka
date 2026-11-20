pub(crate) mod file;
pub(crate) mod folder;
pub(crate) mod group;
pub(crate) mod invite;
pub(crate) mod item;
mod key;
mod key_wrapping;
mod rotated;
pub(crate) mod secure_link;
mod tag;
pub(crate) mod vault;

pub use key::EncryptionKey;
pub(crate) use rotated::RotatedKey;
pub use rotated::{FolderKey, ItemKey, ShareKey, ShareKeys};
pub use tag::EncryptionTag;

use crate::error::{Error, Result};

pub(crate) fn decode_base64(subject: &'static str, value: &str) -> Result<Vec<u8>> {
    rikka_proton_api::base64::decode(value).map_err(|_| Error::InvalidBase64 { subject })
}

pub(crate) fn encode_base64(value: impl AsRef<[u8]>) -> String {
    rikka_proton_api::base64::encode(value)
}
