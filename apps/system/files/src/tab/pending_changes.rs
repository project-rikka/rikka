use std::collections::HashSet;

use files_core::Location;
use files_vfs::Change;

#[derive(Debug, Default)]
pub(crate) struct PendingChanges {
    listing: bool,
    entries: HashSet<Location>,
    waiting: bool,
}

#[derive(Debug)]
pub(crate) enum ChangeSet {
    Listing,
    Entries(Vec<Location>),
}

impl PendingChanges {
    pub(crate) fn record_change(&mut self, change: Change) -> bool {
        match change {
            Change::Listing { .. } => self.listing = true,
            Change::Entry { at } => {
                self.entries.insert(at);
            }
        }
        !std::mem::replace(&mut self.waiting, true)
    }

    pub(crate) fn settle(&mut self) -> ChangeSet {
        let pending_changes = std::mem::take(self);
        match pending_changes.listing {
            true => ChangeSet::Listing,
            false => ChangeSet::Entries(pending_changes.entries.into_iter().collect()),
        }
    }
}
