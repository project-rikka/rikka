use std::collections::HashMap;

use browser_store::{
    BookmarkLink, BookmarkRoots, BookmarkSync, FavoriteId, LinkedItem, PinnedItem, PinnedParent,
    PinnedPlace,
};
use rikka_web::{BookmarkId, BookmarkNode};

use crate::error::Error;
use crate::layout::{FAVORITES, Layout, RIKKA, SPACES};
use crate::merge::{self, Entry, Holder, Merged, Present};
use crate::mirror::{Mirror, MirrorNode};
use crate::remote::Remote;
use crate::shadow::Shadow;
use crate::side::LocalSide;
use crate::sides::{self, Kept, Kind};

#[derive(Debug)]
pub struct Plan {
    other: BookmarkId,
    rikka: Option<BookmarkId>,
    favorites: Option<BookmarkId>,
    spaces: Option<BookmarkId>,
    remote_children: HashMap<BookmarkId, Vec<BookmarkId>>,
    local_children: HashMap<PinnedParent, Vec<PinnedItem>>,
    local_favorites: Vec<FavoriteId>,
    merged: Merged,
}

#[derive(Debug)]
pub struct Pending {
    plan: Plan,
    locals: Vec<Option<LinkedItem>>,
}

impl Plan {
    pub fn new(
        tree: &[BookmarkNode],
        mirror: &Mirror,
        synced: Option<&BookmarkSync>,
    ) -> Result<Plan, Error> {
        let roots = synced.map(|synced| &synced.roots);
        let layout = Layout::locate(tree, roots)?;
        let kept = Kept {
            favorites: Layout::kept(layout.favorites, roots.map(|roots| &roots.favorites)),
            spaces: Layout::kept(layout.spaces, roots.map(|roots| &roots.spaces)),
        };
        let remote = sides::remote(layout.favorites, layout.spaces);
        let local = sides::local(mirror);
        let base = sides::base(synced, &kept, &remote);
        let mut remote_children = HashMap::new();
        for node in tree {
            collect_children(node, &mut remote_children);
        }
        let mut local_children = HashMap::new();
        for space in &mirror.spaces {
            collect_pinned(
                PinnedParent::Root(space.id),
                &space.pinned,
                &mut local_children,
            );
        }
        Ok(Plan {
            other: layout.other.id.clone(),
            rikka: Layout::id(layout.rikka),
            favorites: Layout::id(layout.favorites),
            spaces: Layout::id(layout.spaces),
            remote_children,
            local_children,
            local_favorites: mirror
                .favorites
                .iter()
                .map(|favorite| favorite.id)
                .collect(),
            merged: merge::merge(&remote, &local, &base),
        })
    }

    pub fn apply_local(self, side: &mut impl LocalSide) -> Pending {
        let locals = self
            .merged
            .entries
            .iter()
            .map(|entry| entry.local.as_ref().map(|local| local.key))
            .collect();
        let mut apply = LocalApply {
            merged: &self.merged,
            side,
            locals,
            shadow: Shadow::new(self.local_children.clone()),
        };
        apply.favorites(&self.local_favorites);
        apply.spaces();
        apply.removals();
        let locals = apply.locals;
        Pending { plan: self, locals }
    }
}

impl Pending {
    pub async fn apply_remote(self, remote: &impl Remote) -> Result<BookmarkSync, Error> {
        let Pending { plan, locals } = self;
        let mut apply = RemoteApply {
            merged: &plan.merged,
            remote,
            remotes: plan
                .merged
                .entries
                .iter()
                .map(|entry| entry.remote.as_ref().map(|remote| remote.key.clone()))
                .collect(),
            shadow: Shadow::new(plan.remote_children.clone()),
        };
        let rikka = apply.folder(plan.rikka.clone(), &plan.other, RIKKA).await?;
        let favorites = apply
            .folder(plan.favorites.clone(), &rikka, FAVORITES)
            .await?;
        let spaces = apply.folder(plan.spaces.clone(), &rikka, SPACES).await?;
        let mut work = apply.spaces(&spaces).await?;
        work.push((Holder::Favorites, favorites.clone()));
        while let Some((holder, parent)) = work.pop() {
            work.extend(apply.children(holder, &parent).await?);
        }
        apply.removals().await?;
        let roots = BookmarkRoots {
            rikka: rikka.as_str().to_owned(),
            favorites: favorites.as_str().to_owned(),
            spaces: spaces.as_str().to_owned(),
        };
        Ok(synced(&plan.merged, roots, &locals, &apply.remotes))
    }
}

fn collect_children(node: &BookmarkNode, into: &mut HashMap<BookmarkId, Vec<BookmarkId>>) {
    if !node.is_folder() {
        return;
    }
    into.insert(
        node.id.clone(),
        node.children.iter().map(|child| child.id.clone()).collect(),
    );
    for child in &node.children {
        collect_children(child, into);
    }
}

fn collect_pinned(
    parent: PinnedParent,
    nodes: &[MirrorNode],
    into: &mut HashMap<PinnedParent, Vec<PinnedItem>>,
) {
    let items = nodes
        .iter()
        .map(|node| match node {
            MirrorNode::Folder { id, children, .. } => {
                collect_pinned(PinnedParent::Folder(*id), children, into);
                PinnedItem::Folder(*id)
            }
            MirrorNode::Pin { id, .. } => PinnedItem::Tab(*id),
        })
        .collect();
    into.insert(parent, items);
}

fn pinned_item(item: LinkedItem) -> Option<PinnedItem> {
    match item {
        LinkedItem::Folder(id) => Some(PinnedItem::Folder(id)),
        LinkedItem::Pin(id) => Some(PinnedItem::Tab(id)),
        LinkedItem::Favorite(_) | LinkedItem::Space(_) => None,
    }
}

fn place(parent: PinnedParent, index: usize) -> PinnedPlace {
    let place = match parent {
        PinnedParent::Root(space) => PinnedPlace::root(space),
        PinnedParent::Folder(folder) => PinnedPlace::folder(folder),
    };
    place.at(index)
}

fn changed<K>(present: &Present<K>, entry: &Entry) -> bool {
    present.title != entry.title || present.url != entry.url
}

struct LocalApply<'a, S> {
    merged: &'a Merged,
    side: &'a mut S,
    locals: Vec<Option<LinkedItem>>,
    shadow: Shadow<PinnedParent, PinnedItem>,
}

impl<S: LocalSide> LocalApply<'_, S> {
    fn favorites(&mut self, current: &[FavoriteId]) {
        let mut current = current.to_vec();
        for entry in self.merged.entries.iter().filter(|entry| !entry.alive()) {
            if let Some(LinkedItem::Favorite(id)) = entry.local.as_ref().map(|local| local.key) {
                self.side.remove_favorite(id);
                current.retain(|kept| *kept != id);
            }
        }
        let order = self.merged.order(Holder::Favorites);
        for &at in order {
            let entry = &self.merged.entries[at];
            let url = entry.url.as_deref().unwrap_or_default();
            match &entry.local {
                Some(local) => {
                    if let LinkedItem::Favorite(id) = local.key
                        && changed(local, entry)
                    {
                        self.side.edit_favorite(id, url, &entry.title);
                    }
                }
                None => {
                    if let Some(id) = self.side.add_favorite(url, &entry.title) {
                        self.locals[at] = Some(LinkedItem::Favorite(id));
                        current.push(id);
                    }
                }
            }
        }
        let wanted: Vec<FavoriteId> = order
            .iter()
            .filter_map(|&at| match self.locals[at] {
                Some(LinkedItem::Favorite(id)) => Some(id),
                _ => None,
            })
            .collect();
        if wanted != current {
            self.side.order_favorites(&wanted);
        }
    }

    fn spaces(&mut self) {
        for &at in self.merged.order(Holder::Spaces) {
            let entry = &self.merged.entries[at];
            let space = match &entry.local {
                Some(local) => match local.key {
                    LinkedItem::Space(id) => {
                        if local.title != entry.title {
                            self.side.rename_space(id, &entry.title);
                        }
                        Some(id)
                    }
                    _ => None,
                },
                None => self.side.add_space(&entry.title).inspect(|id| {
                    self.locals[at] = Some(LinkedItem::Space(*id));
                }),
            };
            if let Some(space) = space {
                self.pinned(Holder::Entry(at), PinnedParent::Root(space));
            }
        }
    }

    fn pinned(&mut self, holder: Holder, parent: PinnedParent) {
        let mut previous: Option<PinnedItem> = None;
        for &at in self.merged.order(holder) {
            let entry = &self.merged.entries[at];
            let existing = entry
                .local
                .as_ref()
                .and_then(|local| pinned_item(local.key).map(|item| (local, item)));
            let item = match existing {
                Some((local, item)) => {
                    if changed(local, entry) {
                        self.edit(item, entry);
                    }
                    if self.shadow.out_of_order(&parent, &item, previous.as_ref()) {
                        self.shadow.remove(&item);
                        let index = self.shadow.after(&parent, previous.as_ref());
                        self.shadow.insert(parent, index, item);
                        self.side.move_pinned(item, place(parent, index));
                    }
                    item
                }
                None => match self.create(at, parent, previous.as_ref()) {
                    Some(item) => item,
                    None => continue,
                },
            };
            previous = Some(item);
            if let PinnedItem::Folder(folder) = item {
                self.pinned(Holder::Entry(at), PinnedParent::Folder(folder));
            }
        }
    }

    fn edit(&mut self, item: PinnedItem, entry: &Entry) {
        match item {
            PinnedItem::Folder(id) => self.side.rename_folder(id, &entry.title),
            PinnedItem::Tab(id) => {
                let url = entry.url.as_deref().unwrap_or_default();
                self.side.edit_pin(id, url, &entry.title);
            }
        }
    }

    fn create(
        &mut self,
        at: usize,
        parent: PinnedParent,
        previous: Option<&PinnedItem>,
    ) -> Option<PinnedItem> {
        let entry = &self.merged.entries[at];
        let index = self.shadow.after(&parent, previous);
        let (item, linked) = match entry.kind {
            Kind::Folder => {
                let id = self.side.add_folder(place(parent, index), &entry.title)?;
                (PinnedItem::Folder(id), LinkedItem::Folder(id))
            }
            Kind::Pin | Kind::Favorite | Kind::Space => {
                let url = entry.url.as_deref().unwrap_or_default();
                let id = self.side.add_pin(place(parent, index), url, &entry.title)?;
                (PinnedItem::Tab(id), LinkedItem::Pin(id))
            }
        };
        self.shadow.insert(parent, index, item);
        self.locals[at] = Some(linked);
        Some(item)
    }

    fn removals(&mut self) {
        for entry in &self.merged.entries {
            if entry.alive() || self.merged.is_dead_with_local(entry.local_up) {
                continue;
            }
            if let Some(item) = entry
                .local
                .as_ref()
                .and_then(|local| pinned_item(local.key))
            {
                self.side.remove_pinned(item);
            }
        }
    }
}

struct RemoteApply<'a, R> {
    merged: &'a Merged,
    remote: &'a R,
    remotes: Vec<Option<BookmarkId>>,
    shadow: Shadow<BookmarkId, BookmarkId>,
}

impl<R: Remote> RemoteApply<'_, R> {
    async fn folder(
        &mut self,
        found: Option<BookmarkId>,
        parent: &BookmarkId,
        title: &str,
    ) -> Result<BookmarkId, Error> {
        if let Some(found) = found {
            return Ok(found);
        }
        let index = self.shadow.len(parent);
        let id = self
            .remote
            .create(parent, position(index), title, None)
            .await?;
        self.shadow.insert(parent.clone(), index, id.clone());
        Ok(id)
    }

    async fn spaces(&mut self, parent: &BookmarkId) -> Result<Vec<(Holder, BookmarkId)>, Error> {
        let mut work = Vec::new();
        for &at in self.merged.order(Holder::Spaces) {
            let entry = &self.merged.entries[at];
            let id = match self.remotes[at].clone() {
                Some(id) => {
                    self.update(&id, entry).await?;
                    id
                }
                None => {
                    let index = self.shadow.len(parent);
                    let id = self
                        .remote
                        .create(parent, position(index), &entry.title, None)
                        .await?;
                    self.shadow.insert(parent.clone(), index, id.clone());
                    self.remotes[at] = Some(id.clone());
                    id
                }
            };
            work.push((Holder::Entry(at), id));
        }
        Ok(work)
    }

    async fn children(
        &mut self,
        holder: Holder,
        parent: &BookmarkId,
    ) -> Result<Vec<(Holder, BookmarkId)>, Error> {
        let mut work = Vec::new();
        let mut previous: Option<BookmarkId> = None;
        for &at in self.merged.order(holder) {
            let entry = &self.merged.entries[at];
            let id = match self.remotes[at].clone() {
                Some(id) => {
                    self.update(&id, entry).await?;
                    if self.shadow.out_of_order(parent, &id, previous.as_ref()) {
                        self.relocate(&id, parent, previous.as_ref()).await?;
                    }
                    id
                }
                None => {
                    let index = self.shadow.after(parent, previous.as_ref());
                    let id = self
                        .remote
                        .create(parent, position(index), &entry.title, entry.url.as_deref())
                        .await?;
                    self.shadow.insert(parent.clone(), index, id.clone());
                    self.remotes[at] = Some(id.clone());
                    id
                }
            };
            if entry.kind.holds_children() {
                work.push((Holder::Entry(at), id.clone()));
            }
            previous = Some(id);
        }
        Ok(work)
    }

    async fn update(&self, id: &BookmarkId, entry: &Entry) -> Result<(), Error> {
        let Some(remote) = &entry.remote else {
            return Ok(());
        };
        if remote.title != entry.title {
            self.remote.rename(id, &entry.title).await?;
        }
        if remote.url != entry.url
            && let Some(url) = &entry.url
        {
            self.remote.set_url(id, url).await?;
        }
        Ok(())
    }

    async fn relocate(
        &mut self,
        id: &BookmarkId,
        parent: &BookmarkId,
        previous: Option<&BookmarkId>,
    ) -> Result<(), Error> {
        let from = self.shadow.remove(id);
        let target = self.shadow.after(parent, previous);
        let index = match from {
            Some((from, old)) if from == *parent && target >= old => target + 1,
            _ => target,
        };
        self.remote.move_to(id, parent, position(index)).await?;
        self.shadow.insert(parent.clone(), target, id.clone());
        Ok(())
    }

    async fn removals(&mut self) -> Result<(), Error> {
        for entry in &self.merged.entries {
            if entry.alive() || self.merged.is_dead_with_remote(entry.remote_up) {
                continue;
            }
            if let Some(remote) = &entry.remote {
                self.remote.remove(&remote.key).await?;
                self.shadow.remove(&remote.key);
            }
        }
        Ok(())
    }
}

fn position(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

fn synced(
    merged: &Merged,
    roots: BookmarkRoots,
    locals: &[Option<LinkedItem>],
    remotes: &[Option<BookmarkId>],
) -> BookmarkSync {
    let holders = [
        (Holder::Favorites, Some(roots.favorites.clone())),
        (Holder::Spaces, Some(roots.spaces.clone())),
    ]
    .into_iter()
    .chain(
        merged
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.alive() && entry.kind.holds_children())
            .map(|(at, _)| {
                let parent = remotes[at].as_ref().map(|id| id.as_str().to_owned());
                (Holder::Entry(at), parent)
            }),
    );
    let mut links = Vec::new();
    for (holder, parent) in holders {
        let Some(parent) = parent else {
            continue;
        };
        for (position, &at) in (0_u32..).zip(merged.order(holder)) {
            let entry = &merged.entries[at];
            if let (Some(item), Some(bookmark)) = (locals[at], &remotes[at]) {
                links.push(BookmarkLink {
                    bookmark: bookmark.as_str().to_owned(),
                    item,
                    parent: parent.clone(),
                    position,
                    title: entry.title.clone(),
                    url: entry.url.clone(),
                });
            }
        }
    }
    BookmarkSync { roots, links }
}
