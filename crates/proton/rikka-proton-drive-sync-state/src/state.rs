use std::collections::HashMap;
use std::hash::Hash;
use std::path::PathBuf;

use crate::item::{LocalItem, RemoteItem, SyncedItem};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub local: Vec<LocalItem>,
    pub local_removed: Vec<u64>,
    pub remote: Vec<RemoteItem>,
    pub remote_removed: Vec<String>,
    pub synced: Vec<SyncedItem>,
    pub synced_removed: Vec<i64>,
}

impl Changes {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.local.is_empty()
            && self.local_removed.is_empty()
            && self.remote.is_empty()
            && self.remote_removed.is_empty()
            && self.synced.is_empty()
            && self.synced_removed.is_empty()
    }

    pub fn merge(&mut self, other: Changes) {
        self.local.extend(other.local);
        self.local_removed.extend(other.local_removed);
        self.remote.extend(other.remote);
        self.remote_removed.extend(other.remote_removed);
        self.synced.extend(other.synced);
        self.synced_removed.extend(other.synced_removed);
    }
}

#[derive(Debug, Clone)]
pub struct State {
    local_root: u64,
    remote_root: String,
    local: HashMap<u64, LocalItem>,
    remote: HashMap<String, RemoteItem>,
    synced: HashMap<i64, SyncedItem>,
    next_synced: i64,
}

impl State {
    #[must_use]
    pub fn new(local_root: u64, remote_root: impl Into<String>) -> State {
        State {
            local_root,
            remote_root: remote_root.into(),
            local: HashMap::new(),
            remote: HashMap::new(),
            synced: HashMap::new(),
            next_synced: 1,
        }
    }

    #[must_use]
    pub fn with_items(
        mut self,
        local: impl IntoIterator<Item = LocalItem>,
        remote: impl IntoIterator<Item = RemoteItem>,
        synced: impl IntoIterator<Item = SyncedItem>,
    ) -> State {
        self.local
            .extend(local.into_iter().map(|item| (item.id, item)));
        self.remote
            .extend(remote.into_iter().map(|item| (item.id.clone(), item)));
        self.synced
            .extend(synced.into_iter().map(|item| (item.id, item)));
        self.next_synced = self.synced.keys().max().map_or(1, |max_id| max_id + 1);
        self
    }

    #[must_use]
    pub fn local_root(&self) -> u64 {
        self.local_root
    }

    #[must_use]
    pub fn remote_root(&self) -> &str {
        &self.remote_root
    }

    #[must_use]
    pub fn local(&self) -> &HashMap<u64, LocalItem> {
        &self.local
    }

    #[must_use]
    pub fn remote(&self) -> &HashMap<String, RemoteItem> {
        &self.remote
    }

    #[must_use]
    pub fn synced(&self) -> &HashMap<i64, SyncedItem> {
        &self.synced
    }

    pub fn allocate_synced_id(&mut self) -> i64 {
        let id = self.next_synced;
        self.next_synced += 1;
        id
    }

    pub fn apply(&mut self, changes: &Changes) {
        for id in &changes.local_removed {
            self.local.remove(id);
        }
        for id in &changes.remote_removed {
            self.remote.remove(id);
        }
        for id in &changes.synced_removed {
            self.synced.remove(id);
        }
        for item in &changes.local {
            self.local.insert(item.id, item.clone());
        }
        for item in &changes.remote {
            self.remote.insert(item.id.clone(), item.clone());
        }
        for item in &changes.synced {
            self.next_synced = self.next_synced.max(item.id + 1);
            self.synced.insert(item.id, item.clone());
        }
    }

    #[must_use]
    pub fn local_path(&self, id: u64) -> Option<PathBuf> {
        let mut names = Vec::new();
        let mut current = id;
        while current != self.local_root {
            let item = self.local.get(&current)?;
            names.push(item.name.as_str());
            current = item.parent;
            if names.len() > self.local.len() {
                return None;
            }
        }
        Some(names.iter().rev().collect())
    }

    #[must_use]
    pub fn remote_path(&self, id: &str) -> Option<PathBuf> {
        let mut names = Vec::new();
        let mut current = id;
        while current != self.remote_root {
            let item = self.remote.get(current)?;
            names.push(crate::names::local_name(&item.name));
            current = &item.parent;
            if names.len() > self.remote.len() {
                return None;
            }
        }
        Some(names.iter().rev().collect())
    }

    #[must_use]
    pub fn local_subtree(&self, id: u64) -> Vec<u64> {
        let children = children_by_parent(self.local.values().map(|item| (item.parent, item.id)));
        subtree(&children, id)
    }

    #[must_use]
    pub fn remote_subtree(&self, id: &str) -> Vec<String> {
        let children = children_by_parent(
            self.remote
                .values()
                .map(|item| (item.parent.as_str(), item.id.as_str())),
        );
        subtree(&children, id)
            .into_iter()
            .map(str::to_owned)
            .collect()
    }
}

fn children_by_parent<K: Hash + Eq + Copy>(
    pairs: impl Iterator<Item = (K, K)>,
) -> HashMap<K, Vec<K>> {
    let mut children: HashMap<K, Vec<K>> = HashMap::new();
    for (parent, child) in pairs {
        children.entry(parent).or_default().push(child);
    }
    children
}

fn subtree<K: Hash + Eq + Copy>(children: &HashMap<K, Vec<K>>, root: K) -> Vec<K> {
    let mut nodes = vec![root];
    let mut next = 0;
    while let Some(&node) = nodes.get(next) {
        if let Some(child_ids) = children.get(&node) {
            nodes.extend(child_ids.iter().copied());
        }
        next += 1;
    }
    nodes
}
