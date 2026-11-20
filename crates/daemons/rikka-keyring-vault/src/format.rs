use std::collections::BTreeMap;

use rikka_file_format::JsonFormat;
use serde::{Deserialize, Serialize};

pub(crate) const FORMAT: JsonFormat = JsonFormat::new("rikka-keyring");
pub(crate) const ARGON2ID: &str = "argon2id";
pub(crate) const ARGON2_VERSION: u32 = 0x13;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileRecord {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) created: u64,
    pub(crate) modified: u64,
    pub(crate) slots: Vec<SlotRecord>,
    #[serde(with = "base64_bytes")]
    pub(crate) nonce: Vec<u8>,
    #[serde(with = "base64_bytes")]
    pub(crate) sealed: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum SlotRecord {
    Password {
        kdf: KdfRecord,
        #[serde(with = "base64_bytes")]
        nonce: Vec<u8>,
        #[serde(with = "base64_bytes")]
        sealed: Vec<u8>,
    },
    Wrapped {
        by: String,
        #[serde(with = "base64_bytes")]
        nonce: Vec<u8>,
        #[serde(with = "base64_bytes")]
        sealed: Vec<u8>,
    },
    Bound {
        binding: String,
        #[serde(with = "base64_bytes")]
        blob: Vec<u8>,
        #[serde(with = "base64_bytes")]
        nonce: Vec<u8>,
        #[serde(with = "base64_bytes")]
        sealed: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize)]
pub(crate) struct KdfRecord {
    pub(crate) algorithm: String,
    pub(crate) version: u32,
    pub(crate) memory_kib: u32,
    pub(crate) passes: u32,
    pub(crate) lanes: u32,
    #[serde(with = "base64_bytes")]
    pub(crate) salt: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ContentsRecord {
    pub(crate) next: u64,
    pub(crate) items: Vec<ItemRecord>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ItemRecord {
    pub(crate) id: u64,
    pub(crate) label: String,
    pub(crate) attributes: BTreeMap<String, String>,
    pub(crate) content_type: String,
    pub(crate) created: u64,
    pub(crate) modified: u64,
    #[serde(with = "base64_bytes")]
    pub(crate) nonce: Vec<u8>,
    #[serde(with = "base64_bytes")]
    pub(crate) sealed: Vec<u8>,
}

mod base64_bytes {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(deserializer)?;
        STANDARD
            .decode(text.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}
