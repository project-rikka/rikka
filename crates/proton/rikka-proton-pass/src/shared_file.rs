use std::fmt;

use crate::api::{FileChunk, FileResponse};
use crate::content::FileMetadata;
use crate::crypto::EncryptionKey;
use crate::identifier::FileId;

#[derive(Clone)]
pub struct SharedFile {
    token: String,
    response: FileResponse,
    metadata: FileMetadata,
    key: EncryptionKey,
}

impl SharedFile {
    pub(crate) fn new(
        token: String,
        response: FileResponse,
        metadata: FileMetadata,
        key: EncryptionKey,
    ) -> SharedFile {
        SharedFile {
            token,
            response,
            metadata,
            key,
        }
    }

    pub(crate) fn token(&self) -> &str {
        &self.token
    }

    #[must_use]
    pub fn id(&self) -> &FileId {
        self.response.id()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    #[must_use]
    pub fn mime_type(&self) -> Option<&str> {
        self.metadata.mime()
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.response.size()
    }

    #[must_use]
    pub fn chunks(&self) -> &[FileChunk] {
        self.response.chunks()
    }

    #[must_use]
    pub fn key(&self) -> &EncryptionKey {
        &self.key
    }

    #[must_use]
    pub fn response(&self) -> &FileResponse {
        &self.response
    }
}

impl fmt::Debug for SharedFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedFile")
            .field("id", self.id())
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}
