use jiff::Timestamp;
use std::fmt;

use crate::identifier::{ItemId, SecureLinkId, ShareId};
#[derive(Clone)]
pub struct SecureLink {
    pub(crate) id: SecureLinkId,
    pub(crate) share: ShareId,
    pub(crate) item: ItemId,
    pub(crate) address: String,
    pub(crate) active: bool,
    pub(crate) expiration_time: Timestamp,
    pub(crate) read_count: i64,
    pub(crate) max_read_count: Option<i64>,
}

impl SecureLink {
    #[must_use]
    pub fn id(&self) -> &SecureLinkId {
        &self.id
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
    pub fn address(&self) -> &str {
        &self.address
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[must_use]
    pub fn expiration_time(&self) -> Timestamp {
        self.expiration_time
    }

    #[must_use]
    pub fn read_count(&self) -> i64 {
        self.read_count
    }

    #[must_use]
    pub fn max_read_count(&self) -> Option<i64> {
        self.max_read_count
    }
}

impl fmt::Debug for SecureLink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecureLink")
            .field("id", &self.id)
            .field("item", &self.item)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}
