use std::fmt;

use crate::api::{ItemRevision, ItemState, ItemTarget};
use crate::content::{Item, ItemKind};
use crate::crypto::ItemKey;
use crate::identifier::{FolderId, ItemId, ShareId};
use crate::item_flags::ItemFlags;

#[derive(Clone)]
pub struct PassItem {
    share: ShareId,
    revision: ItemRevision,
    content: Item,
    key: ItemKey,
}

impl PassItem {
    pub(crate) fn new(
        share: ShareId,
        revision: ItemRevision,
        content: Item,
        key: ItemKey,
    ) -> PassItem {
        PassItem {
            share,
            revision,
            content,
            key,
        }
    }

    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share
    }

    #[must_use]
    pub fn id(&self) -> &ItemId {
        self.revision.item_id()
    }

    #[must_use]
    pub fn revision(&self) -> i64 {
        self.revision.revision()
    }

    #[must_use]
    pub fn content(&self) -> &Item {
        &self.content
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.content.name()
    }

    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.content.note()
    }

    #[must_use]
    pub fn kind(&self) -> Option<ItemKind> {
        self.content.kind()
    }

    #[must_use]
    pub fn state(&self) -> ItemState {
        self.revision.state()
    }

    #[must_use]
    pub fn is_trashed(&self) -> bool {
        self.revision.state() == ItemState::Trashed
    }

    #[must_use]
    pub fn folder_id(&self) -> Option<&FolderId> {
        self.revision.folder_id()
    }

    #[must_use]
    pub fn alias_email(&self) -> Option<&str> {
        self.revision.alias_email()
    }

    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.revision.is_pinned()
    }

    #[must_use]
    pub fn flags(&self) -> ItemFlags {
        ItemFlags::new(self.revision.flags())
    }

    #[must_use]
    pub fn key(&self) -> &ItemKey {
        &self.key
    }

    #[must_use]
    pub fn target(&self) -> ItemTarget {
        ItemTarget::new(self.id().clone(), self.revision())
    }

    #[must_use]
    pub fn response(&self) -> &ItemRevision {
        &self.revision
    }

    #[must_use]
    pub fn into_content(self) -> Item {
        self.content
    }
}

impl fmt::Debug for PassItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PassItem")
            .field("share", &self.share)
            .field("id", self.id())
            .field("revision", &self.revision())
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}
