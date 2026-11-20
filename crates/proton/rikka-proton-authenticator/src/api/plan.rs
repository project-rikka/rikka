use std::collections::{HashMap, HashSet};

use crate::api::entry::RemoteEntry;
use crate::api::local::{EntryState, LocalEntry};
use crate::entry::Entry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncAction {
    Upsert,
    Push,
    DeleteLocal,
    DeleteEverywhere,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncStep {
    pub remote_id: Option<String>,
    pub revision: Option<u32>,
    pub entry: Entry,
    pub action: SyncAction,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncPlan {
    steps: Vec<SyncStep>,
}

impl SyncPlan {
    #[must_use]
    pub fn compute(remote: &[RemoteEntry], local: &[LocalEntry]) -> SyncPlan {
        let local_by_id: HashMap<&str, &LocalEntry> = local
            .iter()
            .map(|entry| (entry.entry.id.as_str(), entry))
            .collect();

        let mut steps = Vec::new();
        let mut remote_ids = HashSet::new();

        for entry in remote {
            remote_ids.insert(entry.entry.id.as_str());

            match local_by_id.get(entry.entry.id.as_str()) {
                None => steps.push(upsert(entry)),
                Some(local) => steps.extend(resolve(entry, local)),
            }
        }

        for entry in local {
            if remote_ids.contains(entry.entry.id.as_str()) {
                continue;
            }

            steps.push(SyncStep {
                remote_id: None,
                revision: None,
                entry: entry.entry.clone(),
                action: match entry.state {
                    EntryState::PendingSync => SyncAction::Push,
                    EntryState::Synced | EntryState::PendingDelete => SyncAction::DeleteLocal,
                },
            });
        }

        SyncPlan { steps }
    }

    #[must_use]
    pub fn steps(&self) -> &[SyncStep] {
        &self.steps
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

fn resolve(remote: &RemoteEntry, local: &LocalEntry) -> Option<SyncStep> {
    match local.state {
        EntryState::PendingDelete => Some(step(
            remote,
            remote.entry.clone(),
            SyncAction::DeleteEverywhere,
        )),
        EntryState::Synced => (local.entry != remote.entry).then(|| upsert(remote)),
        EntryState::PendingSync if local.entry == remote.entry => Some(upsert(remote)),
        EntryState::PendingSync => Some(
            match (remote.modified_at == local.modified_at, local.edited_at) {
                (true, Some(_)) => push(remote, local),
                (false, Some(edited)) if remote.modified_at <= edited => push(remote, local),
                _ => upsert(remote),
            },
        ),
    }
}

fn upsert(remote: &RemoteEntry) -> SyncStep {
    step(remote, remote.entry.clone(), SyncAction::Upsert)
}

fn push(remote: &RemoteEntry, local: &LocalEntry) -> SyncStep {
    step(remote, local.entry.clone(), SyncAction::Push)
}

fn step(remote: &RemoteEntry, entry: Entry, action: SyncAction) -> SyncStep {
    SyncStep {
        remote_id: Some(remote.remote_id.clone()),
        revision: Some(remote.revision),
        entry,
        action,
    }
}
