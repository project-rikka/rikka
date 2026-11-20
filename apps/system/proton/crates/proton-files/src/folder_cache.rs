use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use rikka_proton_drive::{File, Folder, LinkId, Node};

use crate::entry::Entry;

const CACHED_FOLDERS: usize = 16;

#[derive(Debug, Default)]
pub(crate) struct FolderCache {
    folders: HashMap<LinkId, Folder>,
    files: HashMap<LinkId, File>,
    listed: HashMap<LinkId, Listed>,
    recent: VecDeque<LinkId>,
}

#[derive(Debug)]
struct Listed {
    entries: Arc<Vec<Entry>>,
    children: Vec<LinkId>,
}

impl FolderCache {
    pub(crate) fn clear(&mut self) {
        *self = FolderCache::default();
    }

    pub(crate) fn folder(&self, id: &LinkId) -> Option<&Folder> {
        self.folders.get(id)
    }

    pub(crate) fn file(&self, id: &LinkId) -> Option<&File> {
        self.files.get(id)
    }

    pub(crate) fn contains(&self, at: &LinkId) -> bool {
        self.listed.contains_key(at)
    }

    pub(crate) fn insert_root(&mut self, root: Folder) {
        self.folders.insert(root.id().clone(), root);
    }

    pub(crate) fn insert(&mut self, nodes: Vec<Node>) -> Vec<LinkId> {
        nodes
            .into_iter()
            .map(|node| match node {
                Node::Folder(folder) => {
                    let id = folder.id().clone();
                    self.folders.insert(id.clone(), folder);
                    id
                }
                Node::File(file) => {
                    let id = file.id().clone();
                    self.files.insert(id.clone(), file);
                    id
                }
            })
            .collect()
    }

    pub(crate) fn entries_mut(&mut self, at: &LinkId) -> Option<&mut Arc<Vec<Entry>>> {
        self.touch(at);
        self.listed.get_mut(at).map(|listed| &mut listed.entries)
    }

    pub(crate) fn listed(
        &mut self,
        at: LinkId,
        entries: Arc<Vec<Entry>>,
        children: Option<Vec<LinkId>>,
        pinned: &[LinkId],
    ) {
        let previous = self.listed.remove(&at).map(|listed| listed.children);
        let children = match (children, previous) {
            (Some(fresh), Some(previous)) => {
                let cached: HashSet<&LinkId> = fresh.iter().collect();
                let removed: Vec<LinkId> = previous
                    .into_iter()
                    .filter(|id| !cached.contains(id))
                    .collect();
                for id in &removed {
                    self.forget(id, pinned);
                }
                fresh
            }
            (Some(fresh), None) => fresh,
            (None, previous) => previous.unwrap_or_default(),
        };

        self.listed.insert(at.clone(), Listed { entries, children });
        self.touch(&at);
        self.trim(pinned);
    }

    pub(crate) fn prune(&mut self, at: &LinkId, gathered: &[LinkId], pinned: &[LinkId]) {
        let listed: HashSet<&LinkId> = self
            .listed
            .get(at)
            .map(|listed| listed.children.iter().collect())
            .unwrap_or_default();
        let stray: Vec<LinkId> = gathered
            .iter()
            .filter(|id| !listed.contains(id))
            .cloned()
            .collect();
        for id in &stray {
            self.drop_node(id, pinned);
        }
    }

    fn touch(&mut self, at: &LinkId) {
        if self.recent.back() == Some(at) {
            return;
        }
        self.recent.retain(|id| id != at);
        self.recent.push_back(at.clone());
    }

    fn trim(&mut self, pinned: &[LinkId]) {
        while self.listed.len() > CACHED_FOLDERS {
            let Some(oldest) = self
                .recent
                .iter()
                .find(|id| !pinned.contains(id) && self.listed.contains_key(*id))
                .cloned()
            else {
                return;
            };
            self.evict(&oldest, pinned);
        }
    }

    fn forget(&mut self, id: &LinkId, pinned: &[LinkId]) {
        self.drop_node(id, pinned);
        if !pinned.contains(id) {
            self.evict(id, pinned);
        }
    }

    fn evict(&mut self, at: &LinkId, pinned: &[LinkId]) {
        self.recent.retain(|id| id != at);
        let Some(listed) = self.listed.remove(at) else {
            return;
        };
        for child in &listed.children {
            self.drop_node(child, pinned);
        }
    }

    fn drop_node(&mut self, id: &LinkId, pinned: &[LinkId]) {
        if pinned.contains(id) {
            return;
        }
        self.folders.remove(id);
        self.files.remove(id);
    }
}
