use std::fmt;

use crate::content::FileMetadata;
use crate::crypto::EncryptionKey;
use crate::identifier::FileId;

#[derive(Clone)]
pub struct PendingFile {
    id: FileId,
    metadata: FileMetadata,
    size: u64,
    chunks: usize,
    key: EncryptionKey,
}

impl PendingFile {
    pub(crate) fn new(
        id: FileId,
        metadata: FileMetadata,
        size: u64,
        chunks: usize,
        key: EncryptionKey,
    ) -> PendingFile {
        PendingFile {
            id,
            metadata,
            size,
            chunks,
            key,
        }
    }

    pub(crate) fn with_metadata(&self, metadata: FileMetadata) -> PendingFile {
        PendingFile {
            metadata,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn id(&self) -> &FileId {
        &self.id
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
        self.size
    }

    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunks
    }

    #[must_use]
    pub fn key(&self) -> &EncryptionKey {
        &self.key
    }
}

impl fmt::Debug for PendingFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingFile")
            .field("id", &self.id)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}
