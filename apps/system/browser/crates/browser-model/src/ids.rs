pub use browser_store::{FavoriteId, FolderId, PinnedItem, PinnedTabId, ProfileId, SpaceId, TabId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

#[derive(Debug, Default)]
pub(crate) struct WindowIds {
    issued: u64,
}

impl WindowIds {
    pub(crate) fn next(&mut self) -> WindowId {
        self.issued += 1;
        WindowId(self.issued)
    }
}
