use std::collections::HashMap;

use jiff::Timestamp;

use crate::api::entry::RemoteEntry;
use crate::api::local::LocalEntry;
use crate::api::outcome::Placement;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncOrder {
    placements: Vec<Placement>,
}

impl SyncOrder {
    #[must_use]
    pub fn compute(remote: &[RemoteEntry], local: &[LocalEntry]) -> SyncOrder {
        let local_by_id: HashMap<&str, &LocalEntry> = local
            .iter()
            .map(|entry| (entry.entry.id.as_str(), entry))
            .collect();

        let mut placements: Vec<Placement> = remote
            .iter()
            .filter_map(|remote| {
                let local = local_by_id.get(remote.entry.id.as_str())?;
                let remote_position = remote.position.unwrap_or(local.position);
                let keeps_local = local.position == remote_position
                    || !local.is_synced()
                    || local.modified_at > remote.modified_at;
                let (position, modified_at) = if keeps_local {
                    (local.position, local.modified_at)
                } else {
                    (remote_position, remote.modified_at)
                };
                Some(Placement {
                    id: local.entry.id.clone(),
                    remote_id: remote.remote_id.clone(),
                    position,
                    modified_at,
                })
            })
            .collect();
        placements.sort_by_key(|placement| placement.position);

        SyncOrder { placements }
    }

    #[must_use]
    pub fn placements(&self) -> &[Placement] {
        &self.placements
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.placements.is_empty()
    }

    #[must_use]
    pub fn starting_position(&self) -> Option<u32> {
        self.placements.first().map(|placement| placement.position)
    }

    #[must_use]
    pub fn remote_ids(&self) -> Vec<String> {
        self.placements
            .iter()
            .map(|placement| placement.remote_id.clone())
            .collect()
    }

    pub(crate) fn placements_modified_at(&self, modified_at: Timestamp) -> Vec<Placement> {
        self.placements
            .iter()
            .map(|placement| Placement {
                modified_at,
                ..placement.clone()
            })
            .collect()
    }

    pub(crate) fn position_of(&self, id: &str) -> Option<u32> {
        self.placements
            .iter()
            .find(|placement| placement.id == id)
            .map(|placement| placement.position)
    }
}
