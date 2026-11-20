use std::fmt;

use rikka_proton_api::parallel_map;
use rikka_proton_pass::PassItem;

use crate::entry::Entry;
use crate::opened::OpenedItem;

#[derive(Clone)]
pub struct DecryptedItem {
    pub entry: Entry,
    pub opened: OpenedItem,
}

impl DecryptedItem {
    #[must_use]
    pub fn from_item(item: &PassItem) -> DecryptedItem {
        DecryptedItem {
            entry: Entry::from_item(item),
            opened: OpenedItem::from_item(item),
        }
    }

    pub(crate) async fn all(items: Vec<PassItem>) -> Vec<DecryptedItem> {
        parallel_map(items, |item| DecryptedItem::from_item(&item)).await
    }
}

impl fmt::Debug for DecryptedItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecryptedItem")
            .field("share", &self.entry.share)
            .field("id", &self.entry.id)
            .field("revision", &self.opened.revision)
            .finish_non_exhaustive()
    }
}
