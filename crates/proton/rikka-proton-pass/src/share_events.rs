use crate::identifier::{EventId, ItemId};
use crate::pass_item::PassItem;
use crate::share::Share;

#[derive(Debug, Clone)]
pub struct ShareEvents {
    pub(crate) share: Option<Share>,
    pub(crate) updated: Vec<PassItem>,
    pub(crate) deleted: Vec<ItemId>,
    pub(crate) new_key_rotation: Option<i64>,
    pub(crate) latest_event: EventId,
    pub(crate) pending: bool,
}

impl ShareEvents {
    #[must_use]
    pub fn share(&self) -> Option<&Share> {
        self.share.as_ref()
    }

    #[must_use]
    pub fn updated(&self) -> &[PassItem] {
        &self.updated
    }

    #[must_use]
    pub fn deleted(&self) -> &[ItemId] {
        &self.deleted
    }

    #[must_use]
    pub fn new_key_rotation(&self) -> Option<i64> {
        self.new_key_rotation
    }

    #[must_use]
    pub fn latest_event(&self) -> &EventId {
        &self.latest_event
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending
    }

    #[must_use]
    pub fn into_updated(self) -> Vec<PassItem> {
        self.updated
    }
}
