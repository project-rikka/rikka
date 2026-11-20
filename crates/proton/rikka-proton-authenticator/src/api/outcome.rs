use jiff::Timestamp;

use crate::api::entry::RemoteEntry;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub id: String,
    pub remote_id: String,
    pub position: u32,
    pub modified_at: Timestamp,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncOutcome {
    pub entries: Vec<RemoteEntry>,
    pub removed: Vec<String>,
    pub order: Vec<Placement>,
    pub undecryptable: usize,
}
