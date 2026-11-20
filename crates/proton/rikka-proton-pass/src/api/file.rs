use jiff::Timestamp;
use serde::{Deserialize, Deserializer, Serialize};

use crate::api::ItemRevision;
use crate::identifier::{ChunkId, FileId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    #[serde(rename = "ChunkID")]
    id: ChunkId,
    #[serde(rename = "Index", default)]
    index: usize,
    #[serde(rename = "Size", default)]
    size: u64,
}

impl FileChunk {
    #[must_use]
    pub fn id(&self) -> &ChunkId {
        &self.id
    }

    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResponse {
    #[serde(rename = "FileID")]
    id: FileId,
    #[serde(rename = "PersistentFileUID", default)]
    uid: String,
    #[serde(rename = "Size", default)]
    size: u64,
    #[serde(rename = "Metadata", default)]
    metadata: String,
    #[serde(rename = "FileKey", default)]
    file_key: String,
    #[serde(rename = "EncryptionVersion", default)]
    encryption_version: Option<i32>,
    #[serde(
        rename = "ItemKeyRotation",
        default,
        deserialize_with = "number_or_text"
    )]
    item_key_rotation: i64,
    #[serde(rename = "Chunks", default)]
    chunks: Vec<FileChunk>,
    #[serde(rename = "RevisionAdded", default)]
    revision_added: i64,
    #[serde(
        rename = "RevisionRemoved",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    revision_removed: Option<i64>,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(
        rename = "ModifyTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    modify_time: Timestamp,
}

impl FileResponse {
    #[must_use]
    pub fn id(&self) -> &FileId {
        &self.id
    }

    #[must_use]
    pub fn uid(&self) -> &str {
        &self.uid
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub fn metadata(&self) -> &str {
        &self.metadata
    }

    #[must_use]
    pub fn file_key(&self) -> &str {
        &self.file_key
    }

    #[must_use]
    pub fn encryption_version(&self) -> i32 {
        self.encryption_version.unwrap_or(1)
    }

    #[must_use]
    pub fn item_key_rotation(&self) -> i64 {
        self.item_key_rotation
    }

    #[must_use]
    pub fn chunks(&self) -> &[FileChunk] {
        &self.chunks
    }

    #[must_use]
    pub fn revision_added(&self) -> i64 {
        self.revision_added
    }

    #[must_use]
    pub fn revision_removed(&self) -> Option<i64> {
        self.revision_removed
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn modify_time(&self) -> Timestamp {
        self.modify_time
    }
}

fn number_or_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumberOrText {
        Number(i64),
        Text(String),
    }

    match NumberOrText::deserialize(deserializer)? {
        NumberOrText::Number(number) => Ok(number),
        NumberOrText::Text(text) => text.parse().map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreatePendingFileRequest {
    #[serde(rename = "Metadata")]
    pub(crate) metadata: String,
    #[serde(rename = "ChunkCount")]
    pub(crate) chunk_count: usize,
    #[serde(rename = "EncryptionVersion")]
    pub(crate) encryption_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FileMetadataRequest {
    #[serde(rename = "Metadata")]
    pub(crate) metadata: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LinkFilesRequest {
    #[serde(rename = "ItemRevision")]
    pub(crate) revision: i64,
    #[serde(rename = "FilesToAdd")]
    pub(crate) add: Vec<EncryptedFileKey>,
    #[serde(rename = "FilesToRemove")]
    pub(crate) remove: Vec<FileId>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EncryptedFileKey {
    #[serde(rename = "FileID")]
    pub(crate) id: FileId,
    #[serde(rename = "FileKey")]
    pub(crate) key: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RestoreFilesRequest {
    #[serde(rename = "FilesToRestore")]
    pub(crate) files: Vec<EncryptedFileKey>,
    #[serde(rename = "ItemKeyRotation")]
    pub(crate) item_key_rotation: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RestoredFiles {
    #[serde(rename = "Item")]
    pub(crate) item: ItemRevision,
    #[serde(rename = "Files", default)]
    pub(crate) files: Vec<FileResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FilePage {
    #[serde(rename = "Files", default)]
    pub(crate) files: Vec<FileResponse>,
    #[serde(rename = "LastID", default)]
    pub(crate) last_id: Option<String>,
}
