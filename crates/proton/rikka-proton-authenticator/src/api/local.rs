use jiff::Timestamp;

use crate::entry::Entry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryState {
    Synced,
    PendingSync,
    PendingDelete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalEntry {
    pub entry: Entry,
    pub state: EntryState,
    pub position: u32,
    pub modified_at: Timestamp,
    pub edited_at: Option<Timestamp>,
}

impl LocalEntry {
    #[must_use]
    pub fn new(
        entry: Entry,
        state: EntryState,
        position: u32,
        modified_at: Timestamp,
    ) -> LocalEntry {
        LocalEntry {
            entry,
            state,
            position,
            modified_at,
            edited_at: None,
        }
    }

    #[must_use]
    pub fn with_edited_at(self, edited_at: Timestamp) -> LocalEntry {
        LocalEntry {
            edited_at: Some(edited_at),
            ..self
        }
    }

    #[must_use]
    pub fn is_synced(&self) -> bool {
        self.state == EntryState::Synced
    }
}
