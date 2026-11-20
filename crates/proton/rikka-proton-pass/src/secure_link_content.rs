use jiff::Timestamp;
use std::fmt;

use crate::content::Item;
use crate::shared_file::SharedFile;

#[derive(Clone)]
pub struct SecureLinkContent {
    pub(crate) item: Item,
    pub(crate) content_format_version: i32,
    pub(crate) read_count: i64,
    pub(crate) max_read_count: Option<i64>,
    pub(crate) expiration_time: Timestamp,
    pub(crate) files: Vec<SharedFile>,
}

impl SecureLinkContent {
    #[must_use]
    pub fn item(&self) -> &Item {
        &self.item
    }

    #[must_use]
    pub fn content_format_version(&self) -> i32 {
        self.content_format_version
    }

    #[must_use]
    pub fn read_count(&self) -> i64 {
        self.read_count
    }

    #[must_use]
    pub fn max_read_count(&self) -> Option<i64> {
        self.max_read_count
    }

    #[must_use]
    pub fn expiration_time(&self) -> Timestamp {
        self.expiration_time
    }

    #[must_use]
    pub fn files(&self) -> &[SharedFile] {
        &self.files
    }

    #[must_use]
    pub fn into_item(self) -> Item {
        self.item
    }
}

impl fmt::Debug for SecureLinkContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecureLinkContent")
            .field("files", &self.files.len())
            .field("expiration_time", &self.expiration_time)
            .finish_non_exhaustive()
    }
}
