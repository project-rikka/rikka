use jiff::Timestamp;
use std::fmt;

use crate::new_item::NewItem;

#[must_use]
#[derive(Clone)]
pub struct ImportItem {
    pub(crate) item: NewItem,
    pub(crate) alias_email: Option<String>,
    pub(crate) trashed: bool,
    pub(crate) created_at: Option<Timestamp>,
    pub(crate) modified_at: Option<Timestamp>,
}

impl ImportItem {
    pub fn new(item: NewItem) -> ImportItem {
        ImportItem {
            item,
            alias_email: None,
            trashed: false,
            created_at: None,
            modified_at: None,
        }
    }

    pub fn alias_email(mut self, email: impl Into<String>) -> ImportItem {
        self.alias_email = Some(email.into());
        self
    }

    pub fn in_trash(mut self) -> ImportItem {
        self.trashed = true;
        self
    }

    pub fn created_at(mut self, at: Timestamp) -> ImportItem {
        self.created_at = Some(at);
        self
    }

    pub fn modified_at(mut self, at: Timestamp) -> ImportItem {
        self.modified_at = Some(at);
        self
    }
}

impl fmt::Debug for ImportItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportItem")
            .field("item", &self.item)
            .field("trashed", &self.trashed)
            .finish_non_exhaustive()
    }
}
