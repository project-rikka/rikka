use std::sync::Arc;

use rikka_proton_drive::{File, Folder, LinkId, ShareKeys, VolumeId};

use crate::entry::Entry;
use crate::folder_cache::FolderCache;
use crate::order::{Order, SortKey};
use crate::page::Page;

#[derive(Clone, Debug)]
pub struct Crumb {
    pub id: LinkId,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct Tree {
    share: Option<ShareKeys>,
    memory: FolderCache,
    crumbs: Vec<Crumb>,
    entries: Arc<Vec<Entry>>,
    gathering: Option<PartialListing>,
    order: Order,
}

#[derive(Debug)]
struct PartialListing {
    at: LinkId,
    entries: Vec<Entry>,
    children: Vec<LinkId>,
    progressive: Option<bool>,
}

impl Tree {
    #[must_use]
    pub fn new() -> Tree {
        Tree::default()
    }

    pub fn load_cached_root(&mut self, root: LinkId, name: String, entries: Vec<Entry>) -> bool {
        if !self.crumbs.is_empty() {
            return false;
        }
        self.crumbs = vec![Crumb {
            id: root.clone(),
            name,
        }];
        self.cache_listing(root, entries, None);
        true
    }

    pub fn opened(&mut self, share: ShareKeys, root: Folder, name: String) {
        let id = root.id().clone();
        if self.crumbs.first().map(|crumb| &crumb.id) != Some(&id) {
            self.crumbs = vec![Crumb { id, name }];
            self.gathering = None;
            self.memory.clear();
            self.show_listed();
        }
        self.crumbs.truncate(1);
        self.memory.insert_root(root);
        self.share = Some(share);
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.share.is_some() && !self.crumbs.is_empty()
    }

    #[must_use]
    pub fn share(&self) -> Option<&ShareKeys> {
        self.share.as_ref()
    }

    #[must_use]
    pub fn volume_id(&self) -> Option<&VolumeId> {
        self.share.as_ref().map(ShareKeys::volume_id)
    }

    #[must_use]
    pub fn at(&self) -> Option<&LinkId> {
        self.crumbs.last().map(|crumb| &crumb.id)
    }

    #[must_use]
    pub fn folder(&self) -> Option<&Folder> {
        self.memory.folder(self.at()?)
    }

    #[must_use]
    pub fn file(&self, id: &LinkId) -> Option<&File> {
        self.memory.file(id)
    }

    #[must_use]
    pub fn is_cached(&self, at: &LinkId) -> bool {
        self.memory.contains(at)
    }

    #[must_use]
    pub fn crumbs(&self) -> &[Crumb] {
        &self.crumbs
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.crumbs.len()
    }

    #[must_use]
    pub fn entries(&self) -> &Arc<Vec<Entry>> {
        &self.entries
    }

    #[must_use]
    pub fn entry(&self, at: usize) -> Option<&Entry> {
        self.entries.get(at)
    }

    #[must_use]
    pub fn place(&self, id: &LinkId) -> Option<usize> {
        self.entries.iter().position(|entry| &entry.id == id)
    }

    #[must_use]
    pub fn order(&self) -> Order {
        self.order
    }

    pub fn begin(&mut self, at: &LinkId) {
        let started = PartialListing {
            at: at.clone(),
            entries: Vec::new(),
            children: Vec::new(),
            progressive: None,
        };
        if let Some(abandoned) = self.gathering.replace(started) {
            let pinned = self.pinned();
            self.memory
                .prune(&abandoned.at, &abandoned.children, &pinned);
        }
    }

    pub fn append_page(&mut self, at: &LinkId, page: Page) -> bool {
        let showing = self.at() == Some(at);
        let empty = self.entries.is_empty();
        let order = self.order;
        let Some(gathering) = self.gathering.as_mut().filter(|partial| &partial.at == at) else {
            return false;
        };

        gathering.children.extend(self.memory.insert(page.nodes));
        gathering.entries.extend(page.entries);
        order.arrange(&mut gathering.entries);
        let progressive = *gathering.progressive.get_or_insert(showing && empty);
        if !(progressive && showing) {
            return false;
        }
        self.entries = Arc::new(gathering.entries.clone());
        true
    }

    pub fn finish_listing(&mut self, at: &LinkId) -> bool {
        let Some(gathering) = self.gathering.take_if(|partial| &partial.at == at) else {
            return false;
        };
        self.cache_listing(gathering.at, gathering.entries, Some(gathering.children))
    }

    pub fn listed(&mut self, at: &LinkId, page: Page) -> bool {
        self.begin(at);
        self.append_page(at, page);
        self.finish_listing(at)
    }

    pub fn load_cached(&mut self, at: &LinkId, entries: Vec<Entry>) -> bool {
        let gathering = self
            .gathering
            .as_ref()
            .is_some_and(|partial| &partial.at == at && partial.progressive.is_some());
        if gathering || self.memory.contains(at) {
            return false;
        }
        self.cache_listing(at.clone(), entries, None)
    }

    pub fn remove(&mut self, ids: &[LinkId]) {
        Arc::make_mut(&mut self.entries).retain(|entry| !ids.contains(&entry.id));
        self.keep_shown();
    }

    #[must_use]
    pub fn enter(&mut self, id: &LinkId) -> bool {
        let Some(folder) = self.memory.folder(id) else {
            return false;
        };
        let name = self
            .entries
            .iter()
            .find(|entry| &entry.id == id)
            .map_or_else(|| folder.name().to_owned(), |entry| entry.name.clone());
        self.crumbs.push(Crumb {
            id: id.clone(),
            name,
        });
        self.show_listed();
        true
    }

    pub fn back_to(&mut self, depth: usize) -> bool {
        if depth == 0 || depth >= self.crumbs.len() {
            return false;
        }
        self.crumbs.truncate(depth);
        self.show_listed();
        true
    }

    #[must_use]
    pub fn sorted(&self) -> SortKey {
        self.order.sorted()
    }

    #[must_use]
    pub fn is_descending(&self) -> bool {
        self.order.is_descending()
    }

    pub fn sort(&mut self, sorted: SortKey) {
        self.order = self.order.toggled(sorted);
        let order = self.order;
        order.arrange(Arc::make_mut(&mut self.entries).as_mut_slice());
        if let Some(gathering) = self.gathering.as_mut() {
            order.arrange(&mut gathering.entries);
        }
        self.keep_shown();
    }

    #[must_use]
    pub fn path(&self) -> String {
        self.crumbs
            .iter()
            .map(|crumb| crumb.name.as_str())
            .collect::<Vec<&str>>()
            .join("/")
    }

    fn pinned(&self) -> Vec<LinkId> {
        self.crumbs.iter().map(|crumb| crumb.id.clone()).collect()
    }

    fn keep_shown(&mut self) {
        let Some(at) = self.at().cloned() else {
            return;
        };
        if self.memory.contains(&at) {
            let pinned = self.pinned();
            self.memory
                .listed(at, Arc::clone(&self.entries), None, &pinned);
        }
    }

    fn cache_listing(
        &mut self,
        at: LinkId,
        mut entries: Vec<Entry>,
        children: Option<Vec<LinkId>>,
    ) -> bool {
        self.order.arrange(&mut entries);
        let entries = Arc::new(entries);
        let showing = self.at() == Some(&at);
        if showing {
            self.entries = Arc::clone(&entries);
        }
        let pinned = self.pinned();
        self.memory.listed(at, entries, children, &pinned);
        showing
    }

    fn show_listed(&mut self) {
        let order = self.order;
        let listed = self
            .at()
            .cloned()
            .and_then(|at| self.memory.entries_mut(&at));
        self.entries = match listed {
            Some(entries) => {
                if !order.is_sorted(entries) {
                    order.arrange(Arc::make_mut(entries).as_mut_slice());
                }
                Arc::clone(entries)
            }
            None => Arc::new(Vec::new()),
        };
    }
}
