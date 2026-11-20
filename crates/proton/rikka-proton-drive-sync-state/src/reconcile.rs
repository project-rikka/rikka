use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::item::{Kind, LocalItem, RemoteItem, SyncedItem};
use crate::names::{Conflict, conflict_name, local_name, same_name};
use crate::plan::{Context, Operation, Plan};
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    Delete,
    Wait,
    Revive,
}

#[must_use]
pub fn reconcile(state: &State, context: &Context) -> Plan {
    let mut planner = Planner::new(state, context);
    planner.plan_rebinds();
    planner.plan_deletions();
    planner.plan_changes();
    planner.plan_creations();
    planner.finish()
}

struct Planner<'a> {
    state: &'a State,
    context: &'a Context,
    rows: Vec<&'a SyncedItem>,
    by_local: HashMap<u64, &'a SyncedItem>,
    by_remote: HashMap<&'a str, &'a SyncedItem>,
    local_children: HashMap<u64, Vec<&'a LocalItem>>,
    remote_children: HashMap<&'a str, Vec<&'a RemoteItem>>,
    resolved: HashSet<i64>,
    forgotten: HashSet<i64>,
    claimed_local: HashSet<u64>,
    claimed_remote: HashSet<&'a str>,
    planned: Vec<(u64, Operation)>,
}

impl<'a> Planner<'a> {
    fn new(state: &'a State, context: &'a Context) -> Planner<'a> {
        let mut rows: Vec<&SyncedItem> = state.synced().values().collect();
        rows.sort_by_key(|row| row.id);
        let by_local = rows.iter().map(|row| (row.local, *row)).collect();
        let by_remote = rows.iter().map(|row| (row.remote.as_str(), *row)).collect();

        let mut local_children: HashMap<u64, Vec<&LocalItem>> = HashMap::new();
        for item in state.local().values() {
            local_children.entry(item.parent).or_default().push(item);
        }
        for child in local_children.values_mut() {
            child.sort_by(|one, other| one.name.cmp(&other.name).then(one.id.cmp(&other.id)));
        }
        let mut remote_children: HashMap<&str, Vec<&RemoteItem>> = HashMap::new();
        for item in state.remote().values() {
            remote_children
                .entry(item.parent.as_str())
                .or_default()
                .push(item);
        }
        for child in remote_children.values_mut() {
            child.sort_by(|one, other| one.name.cmp(&other.name).then(one.id.cmp(&other.id)));
        }

        Planner {
            state,
            context,
            rows,
            by_local,
            by_remote,
            local_children,
            remote_children,
            resolved: HashSet::new(),
            forgotten: HashSet::new(),
            claimed_local: HashSet::new(),
            claimed_remote: HashSet::new(),
            planned: Vec::new(),
        }
    }

    fn finish(mut self) -> Plan {
        self.planned
            .sort_by_key(|(order, operation)| (operation.phase(), *order));
        Plan::new(
            self.planned
                .into_iter()
                .map(|(_, operation)| operation)
                .collect(),
        )
    }

    fn plan(&mut self, order: u64, operation: Operation) {
        self.planned.push((order, operation));
    }

    fn local_item(&self, row: &SyncedItem) -> Option<&'a LocalItem> {
        self.state
            .local()
            .get(&row.local)
            .filter(|item| item.kind == row.kind && item.id != self.state.local_root())
    }

    fn remote_item(&self, row: &SyncedItem) -> Option<&'a RemoteItem> {
        self.state
            .remote()
            .get(&row.remote)
            .filter(|item| item.kind == row.kind && item.id != self.state.remote_root())
    }

    fn is_unsynced_local(&self, item: &LocalItem) -> bool {
        self.by_local
            .get(&item.id)
            .is_none_or(|row| row.kind != item.kind || self.forgotten.contains(&row.id))
    }

    fn is_unsynced_remote(&self, item: &RemoteItem) -> bool {
        self.by_remote
            .get(item.id.as_str())
            .is_none_or(|row| row.kind != item.kind || self.forgotten.contains(&row.id))
    }

    fn is_bound(&self, row: &SyncedItem) -> bool {
        !self.forgotten.contains(&row.id)
            && self.local_item(row).is_some()
            && self.remote_item(row).is_some()
    }

    fn remote_parent_for(&self, local_parent: u64) -> Option<&'a str> {
        if local_parent == self.state.local_root() {
            return Some(self.state.remote_root());
        }
        let row = self.by_local.get(&local_parent)?;
        (row.kind == Kind::Folder && self.is_bound(row)).then_some(row.remote.as_str())
    }

    fn local_parent_for(&self, remote_parent: &str) -> Option<u64> {
        if remote_parent == self.state.remote_root() {
            return Some(self.state.local_root());
        }
        let row = self.by_remote.get(remote_parent)?;
        (row.kind == Kind::Folder && self.is_bound(row)).then_some(row.local)
    }

    fn local_moved(row: &SyncedItem, local: &LocalItem) -> bool {
        local.parent != row.local_parent || local.name != row.local_name
    }

    fn remote_moved(row: &SyncedItem, remote: &RemoteItem) -> bool {
        remote.parent != row.remote_parent || remote.name != row.remote_name
    }

    fn local_edited(row: &SyncedItem, local: &LocalItem) -> Option<bool> {
        match row.kind {
            Kind::Folder => Some(false),
            Kind::File => local
                .sha1
                .as_ref()
                .map(|sha1| Some(sha1) != row.sha1.as_ref() || local.size != row.size),
        }
    }

    fn remote_edited(row: &SyncedItem, remote: &RemoteItem) -> bool {
        row.kind == Kind::File
            && remote.revision != row.revision
            && (remote.sha1.is_none() || remote.sha1 != row.sha1)
    }

    fn local_changed(row: &SyncedItem, local: &LocalItem) -> bool {
        Self::local_moved(row, local) || Self::local_edited(row, local) != Some(false)
    }

    fn remote_changed(row: &SyncedItem, remote: &RemoteItem) -> bool {
        Self::remote_moved(row, remote) || Self::remote_edited(row, remote)
    }

    fn forget(&mut self, row: &SyncedItem) {
        self.resolved.insert(row.id);
        if self.forgotten.insert(row.id) {
            self.plan(0, Operation::Forget { synced: row.id });
        }
    }

    fn plan_rebinds(&mut self) {
        for row in self.rows.clone() {
            let local = match self.local_item(row) {
                Some(_) => None,
                None => self.local_replacement(row),
            };
            let remote = match self.remote_item(row) {
                Some(_) => None,
                None => self.remote_replacement(row),
            };
            if local.is_none() && remote.is_none() {
                continue;
            }
            if let Some(local) = local {
                self.claimed_local.insert(local);
            }
            if let Some(remote) = remote {
                self.claimed_remote.insert(remote);
            }
            self.resolved.insert(row.id);
            self.plan(
                0,
                Operation::Rebind {
                    synced: row.id,
                    local,
                    remote: remote.map(str::to_owned),
                },
            );
        }
    }

    fn local_replacement(&self, row: &SyncedItem) -> Option<u64> {
        self.local_children
            .get(&row.local_parent)?
            .iter()
            .find(|item| {
                item.name == row.local_name
                    && item.kind == row.kind
                    && self.is_unsynced_local(item)
                    && !self.claimed_local.contains(&item.id)
            })
            .map(|item| item.id)
    }

    fn remote_replacement(&self, row: &SyncedItem) -> Option<&'a str> {
        self.remote_children
            .get(row.remote_parent.as_str())?
            .iter()
            .find(|item| {
                item.name == row.remote_name
                    && item.kind == row.kind
                    && self.is_unsynced_remote(item)
                    && !self.claimed_remote.contains(item.id.as_str())
            })
            .map(|item| item.id.as_str())
    }

    fn plan_deletions(&mut self) {
        let mut local_removed = Vec::new();
        let mut remote_removed = Vec::new();
        for row in self.rows.clone() {
            if self.resolved.contains(&row.id) {
                continue;
            }
            match (self.local_item(row), self.remote_item(row)) {
                (None, None) => self.forget(row),
                (None, Some(remote)) => local_removed.push((row, remote)),
                (Some(local), None) => remote_removed.push((row, local)),
                (Some(_), Some(_)) => {}
            }
        }

        let removed_remote_folders: HashSet<&str> = local_removed
            .iter()
            .filter(|(row, _)| row.kind == Kind::Folder)
            .map(|(row, _)| row.remote.as_str())
            .collect();
        for (row, remote) in local_removed {
            if self.resolved.contains(&row.id)
                || self
                    .remote_ancestors(&remote.parent)
                    .any(|id| removed_remote_folders.contains(id))
            {
                continue;
            }
            self.plan_local_deletion(row, remote);
        }

        let removed_local_folders: HashSet<u64> = remote_removed
            .iter()
            .filter(|(row, _)| row.kind == Kind::Folder)
            .map(|(row, _)| row.local)
            .collect();
        for (row, local) in remote_removed {
            if self.resolved.contains(&row.id)
                || self
                    .local_ancestors(local.parent)
                    .any(|id| removed_local_folders.contains(&id))
            {
                continue;
            }
            self.plan_remote_deletion(row, local);
        }
    }

    fn plan_local_deletion(&mut self, row: &'a SyncedItem, remote: &'a RemoteItem) {
        if row.kind == Kind::File {
            if Self::remote_changed(row, remote) {
                self.forget(row);
            } else {
                self.resolved.insert(row.id);
                self.plan(0, Operation::TrashRemote { synced: row.id });
            }
            return;
        }

        let mut verdict = match Self::remote_moved(row, remote) {
            true => Verdict::Revive,
            false => Verdict::Delete,
        };
        let mut removed = vec![row];
        for item in self.remote_descendants(&remote.id) {
            match self
                .by_remote
                .get(item.id.as_str())
                .filter(|child| !self.forgotten.contains(&child.id))
            {
                None => verdict = Verdict::Revive,
                Some(child) if self.local_item(child).is_some() => {
                    verdict = verdict.max(Verdict::Wait)
                }
                Some(child) => {
                    if Self::remote_changed(child, item) {
                        verdict = Verdict::Revive;
                    }
                    removed.push(child);
                }
            }
        }

        match verdict {
            Verdict::Delete => {
                self.resolved.extend(removed.iter().map(|child| child.id));
                self.plan(0, Operation::TrashRemote { synced: row.id });
            }
            Verdict::Wait => self.resolved.extend(removed.iter().map(|child| child.id)),
            Verdict::Revive => {
                for child in removed {
                    self.forget(child);
                }
            }
        }
    }

    fn plan_remote_deletion(&mut self, row: &'a SyncedItem, local: &'a LocalItem) {
        if row.kind == Kind::File {
            if Self::local_changed(row, local) {
                self.forget(row);
            } else {
                self.resolved.insert(row.id);
                self.plan(0, Operation::TrashLocal { synced: row.id });
            }
            return;
        }

        let mut verdict = match Self::local_moved(row, local) {
            true => Verdict::Revive,
            false => Verdict::Delete,
        };
        let mut removed = vec![row];
        for item in self.local_descendants(local.id) {
            match self
                .by_local
                .get(&item.id)
                .filter(|child| !self.forgotten.contains(&child.id))
            {
                None => verdict = Verdict::Revive,
                Some(child) if self.remote_item(child).is_some() => {
                    verdict = verdict.max(Verdict::Wait);
                }
                Some(child) => {
                    if Self::local_changed(child, item) {
                        verdict = Verdict::Revive;
                    }
                    removed.push(child);
                }
            }
        }

        match verdict {
            Verdict::Delete => {
                self.resolved.extend(removed.iter().map(|child| child.id));
                self.plan(0, Operation::TrashLocal { synced: row.id });
            }
            Verdict::Wait => self.resolved.extend(removed.iter().map(|child| child.id)),
            Verdict::Revive => {
                for child in removed {
                    self.forget(child);
                }
            }
        }
    }

    fn plan_changes(&mut self) {
        for row in self.rows.clone() {
            if self.resolved.contains(&row.id) {
                continue;
            }
            let (Some(local), Some(remote)) = (self.local_item(row), self.remote_item(row)) else {
                continue;
            };
            self.plan_change(row, local, remote);
        }
    }

    fn plan_change(&mut self, row: &'a SyncedItem, local: &'a LocalItem, remote: &'a RemoteItem) {
        let local_moved = Self::local_moved(row, local);
        let remote_moved = Self::remote_moved(row, remote);
        let local_edited = Self::local_edited(row, local);
        let remote_edited = Self::remote_edited(row, remote);
        let mut refresh = false;
        let mut acted = false;

        if local_edited == Some(true) && remote_edited {
            match local.sha1.is_some() && local.sha1 == remote.sha1 {
                true => refresh = true,
                false => {
                    self.resolved.insert(row.id);
                    self.claimed_local.insert(local.id);
                    let name = conflict_name(
                        &local.name,
                        Conflict::Edit,
                        self.context.device(),
                        self.context.today(),
                    );
                    self.plan(
                        0,
                        Operation::SetAside {
                            local: local.id,
                            name,
                            conflict: Conflict::Edit,
                            forget: Some(row.id),
                        },
                    );
                    return;
                }
            }
        }

        match (local_moved, remote_moved) {
            (true, true) if self.same_place(local, remote) => refresh = true,
            (true, true) | (false, true) => acted |= self.move_local(row, local, remote),
            (true, false) => acted |= self.move_remote(row, local, remote),
            (false, false) => {}
        }

        match (local_edited, remote_edited) {
            (None, _) => {}
            (Some(true), false) => {
                if let Some(sha1) = &local.sha1 {
                    acted = true;
                    self.plan(
                        local.size,
                        Operation::UploadRevision {
                            synced: row.id,
                            sha1: sha1.clone(),
                        },
                    );
                }
            }
            (Some(false), true) => {
                if let Some(revision) = &remote.revision {
                    acted = true;
                    self.plan(
                        remote.size,
                        Operation::DownloadRevision {
                            synced: row.id,
                            revision: revision.clone(),
                        },
                    );
                }
            }
            (Some(false), false) if remote.revision != row.revision => refresh = true,
            (Some(_), _) => {}
        }

        if refresh && !acted {
            self.plan(0, Operation::Refresh { synced: row.id });
        }
    }

    fn same_place(&self, local: &LocalItem, remote: &RemoteItem) -> bool {
        self.remote_parent_for(local.parent) == Some(remote.parent.as_str())
            && same_name(&local.name, &local_name(&remote.name))
    }

    fn move_remote(&mut self, row: &SyncedItem, local: &LocalItem, remote: &RemoteItem) -> bool {
        let Some(parent) = self.remote_parent_for(local.parent) else {
            return false;
        };
        if row.kind == Kind::Folder && self.remote_ancestors(parent).any(|id| id == remote.id) {
            return self.move_local(row, local, remote);
        }
        self.plan(
            0,
            Operation::MoveRemote {
                synced: row.id,
                parent: parent.to_owned(),
                name: local.name.clone(),
            },
        );
        true
    }

    fn move_local(&mut self, row: &SyncedItem, local: &LocalItem, remote: &RemoteItem) -> bool {
        let Some(parent) = self.local_parent_for(&remote.parent) else {
            return false;
        };
        let name = local_name(&remote.name);
        if parent == local.parent && name == local.name {
            return false;
        }
        if row.kind == Kind::Folder && self.local_ancestors(parent).any(|id| id == local.id) {
            return false;
        }
        self.plan(
            0,
            Operation::MoveLocal {
                synced: row.id,
                parent,
                name,
            },
        );
        true
    }

    fn plan_creations(&mut self) {
        let mut locals: Vec<&LocalItem> = self
            .state
            .local()
            .values()
            .filter(|item| {
                item.id != self.state.local_root()
                    && !self.claimed_local.contains(&item.id)
                    && self.is_unsynced_local(item)
            })
            .collect();
        locals.sort_by(|one, other| {
            (one.parent, &one.name, one.id).cmp(&(other.parent, &other.name, other.id))
        });
        for local in locals {
            self.plan_local_creation(local);
        }

        let mut remotes: Vec<&RemoteItem> = self
            .state
            .remote()
            .values()
            .filter(|item| {
                item.id != self.state.remote_root()
                    && !self.claimed_remote.contains(item.id.as_str())
                    && self.is_unsynced_remote(item)
            })
            .collect();
        remotes.sort_by(|one, other| {
            (&one.parent, &one.name, &one.id).cmp(&(&other.parent, &other.name, &other.id))
        });
        for remote in remotes {
            self.plan_remote_creation(remote);
        }
    }

    fn plan_local_creation(&mut self, local: &'a LocalItem) {
        let Some(parent) = self.remote_parent_for(local.parent) else {
            return;
        };
        let file_sha1 = match (local.kind, &local.sha1) {
            (Kind::File, None) => return,
            (Kind::File, Some(sha1)) => Some(sha1),
            (Kind::Folder, _) => None,
        };

        let name_match = self
            .remote_children
            .get(parent)
            .into_iter()
            .flatten()
            .filter(|item| {
                self.is_unsynced_remote(item)
                    && !self.claimed_remote.contains(item.id.as_str())
                    && same_name(&local_name(&item.name), &local.name)
            })
            .min_by_key(|item| (item.kind != local.kind, item.sha1 != local.sha1))
            .copied();

        self.claimed_local.insert(local.id);
        if let Some(name_match) = name_match {
            self.claimed_remote.insert(name_match.id.as_str());
            let same = match (local.kind, name_match.kind) {
                (Kind::Folder, Kind::Folder) => true,
                (Kind::File, Kind::File) => {
                    name_match.sha1.is_some() && name_match.sha1 == local.sha1
                }
                _ => false,
            };
            let operation = match same {
                true => Operation::Link {
                    local: local.id,
                    remote: name_match.id.clone(),
                },
                false => Operation::SetAside {
                    local: local.id,
                    name: conflict_name(
                        &local.name,
                        Conflict::Clash,
                        self.context.device(),
                        self.context.today(),
                    ),
                    conflict: Conflict::Clash,
                    forget: None,
                },
            };
            self.plan(0, operation);
            return;
        }

        let operation = match file_sha1 {
            None => Operation::CreateRemoteFolder {
                local: local.id,
                parent: parent.to_owned(),
                name: local.name.clone(),
            },
            Some(sha1) => Operation::Upload {
                local: local.id,
                parent: parent.to_owned(),
                name: local.name.clone(),
                sha1: sha1.clone(),
            },
        };
        self.plan(local.size, operation);
    }

    fn plan_remote_creation(&mut self, remote: &'a RemoteItem) {
        let Some(parent) = self.local_parent_for(&remote.parent) else {
            return;
        };
        let name = local_name(&remote.name);
        let has_unsynced_local_match = self
            .local_children
            .get(&parent)
            .into_iter()
            .flatten()
            .any(|item| self.is_unsynced_local(item) && same_name(&item.name, &name));
        if has_unsynced_local_match {
            return;
        }

        self.claimed_remote.insert(remote.id.as_str());
        match remote.kind {
            Kind::Folder => self.plan(
                0,
                Operation::CreateLocalFolder {
                    remote: remote.id.clone(),
                    parent,
                    name,
                },
            ),
            Kind::File => {
                if let Some(revision) = &remote.revision {
                    self.plan(
                        remote.size,
                        Operation::Download {
                            remote: remote.id.clone(),
                            parent,
                            name,
                            revision: revision.clone(),
                        },
                    );
                }
            }
        }
    }

    fn remote_descendants(&self, id: &'a str) -> Vec<&'a RemoteItem> {
        descendants(&self.remote_children, id, |item| item.id.as_str())
    }

    fn local_descendants(&self, id: u64) -> Vec<&'a LocalItem> {
        descendants(&self.local_children, id, |item| item.id)
    }

    fn remote_ancestors(&self, start: &'a str) -> impl Iterator<Item = &'a str> + use<'a> {
        let (remote, root) = (self.state.remote(), self.state.remote_root());
        std::iter::successors(Some(start), move |id| {
            (*id != root)
                .then(|| remote.get(*id))
                .flatten()
                .map(|item| item.parent.as_str())
        })
        .take(remote.len() + 1)
    }

    fn local_ancestors(&self, start: u64) -> impl Iterator<Item = u64> + use<'a> {
        let (local, root) = (self.state.local(), self.state.local_root());
        std::iter::successors(Some(start), move |id| {
            (*id != root)
                .then(|| local.get(id))
                .flatten()
                .map(|item| item.parent)
        })
        .take(local.len() + 1)
    }
}

fn descendants<'a, K: Hash + Eq, T>(
    children: &HashMap<K, Vec<&'a T>>,
    root: K,
    key: impl Fn(&'a T) -> K,
) -> Vec<&'a T> {
    let mut found: Vec<&'a T> = children.get(&root).cloned().unwrap_or_default();
    let mut next = 0;
    while let Some(&item) = found.get(next) {
        if let Some(child_items) = children.get(&key(item)) {
            found.extend(child_items.iter().copied());
        }
        next += 1;
    }
    found
}
