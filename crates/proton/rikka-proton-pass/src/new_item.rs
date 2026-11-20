use std::fmt;

use crate::content::Item;
use crate::identifier::FolderId;

#[must_use]
#[derive(Clone)]
pub struct NewItem {
    pub(crate) content: Item,
    pub(crate) folder: Option<FolderId>,
    pub(crate) url_matching: bool,
}

impl NewItem {
    pub fn new(content: Item) -> NewItem {
        NewItem {
            content,
            folder: None,
            url_matching: true,
        }
    }

    pub fn in_folder(mut self, folder: FolderId) -> NewItem {
        self.folder = Some(folder);
        self
    }

    pub fn without_url_matching(mut self) -> NewItem {
        self.url_matching = false;
        self
    }
}

impl fmt::Debug for NewItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewItem")
            .field("folder", &self.folder)
            .field("url_matching", &self.url_matching)
            .finish_non_exhaustive()
    }
}
