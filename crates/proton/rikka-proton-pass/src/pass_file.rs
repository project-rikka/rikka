use std::fmt;

use crate::api::{FileChunk, FileResponse};
use crate::content::FileMetadata;
use crate::crypto::EncryptionKey;
use crate::identifier::{FileId, ItemId, ShareId};

#[derive(Clone)]
pub struct PassFile {
    share: ShareId,
    item: ItemId,
    response: FileResponse,
    metadata: FileMetadata,
    key: EncryptionKey,
}

impl PassFile {
    pub(crate) fn new(
        share: ShareId,
        item: ItemId,
        response: FileResponse,
        metadata: FileMetadata,
        key: EncryptionKey,
    ) -> PassFile {
        PassFile {
            share,
            item,
            response,
            metadata,
            key,
        }
    }

    #[must_use]
    pub fn id(&self) -> &FileId {
        self.response.id()
    }

    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share
    }

    #[must_use]
    pub fn item_id(&self) -> &ItemId {
        &self.item
    }

    #[must_use]
    pub fn uid(&self) -> &str {
        self.response.uid()
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
    pub fn metadata(&self) -> &FileMetadata {
        &self.metadata
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
    pub fn revision_added(&self) -> i64 {
        self.response.revision_added()
    }

    #[must_use]
    pub fn revision_removed(&self) -> Option<i64> {
        self.response.revision_removed()
    }

    #[must_use]
    pub fn is_present_in(&self, revision: i64) -> bool {
        revision >= self.revision_added()
            && self
                .revision_removed()
                .is_none_or(|removed| revision < removed)
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

impl fmt::Debug for PassFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PassFile")
            .field("id", self.id())
            .field("item", &self.item)
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}
