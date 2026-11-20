use std::fmt;

use crate::api::FolderResponse;
use crate::crypto::FolderKey;
use crate::identifier::{FolderId, ShareId};

#[derive(Clone)]
pub struct PassFolder {
    share: ShareId,
    response: FolderResponse,
    name: String,
    key: FolderKey,
}

impl PassFolder {
    pub(crate) fn new(
        share: ShareId,
        response: FolderResponse,
        name: String,
        key: FolderKey,
    ) -> PassFolder {
        PassFolder {
            share,
            response,
            name,
            key,
        }
    }

    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share
    }

    #[must_use]
    pub fn id(&self) -> &FolderId {
        self.response.folder_id()
    }

    #[must_use]
    pub fn parent_id(&self) -> Option<&FolderId> {
        self.response.parent_folder_id()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn key_rotation(&self) -> i64 {
        self.response.key_rotation()
    }

    #[must_use]
    pub fn key(&self) -> &FolderKey {
        &self.key
    }

    #[must_use]
    pub fn response(&self) -> &FolderResponse {
        &self.response
    }
}

impl fmt::Debug for PassFolder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PassFolder")
            .field("share", &self.share)
            .field("id", self.id())
            .field("parent", &self.parent_id())
            .finish_non_exhaustive()
    }
}
