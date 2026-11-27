use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use browser_store::{BookmarkLink, BookmarkSync, LinkedItem};
use rikka_web::{BookmarkId, BookmarkNode};

use crate::mirror::{Mirror, MirrorNode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Favorite,
    Space,
    Folder,
    Pin,
}

impl Kind {
    pub(crate) fn of(item: LinkedItem) -> Kind {
        match item {
            LinkedItem::Favorite(_) => Kind::Favorite,
            LinkedItem::Space(_) => Kind::Space,
            LinkedItem::Folder(_) => Kind::Folder,
            LinkedItem::Pin(_) => Kind::Pin,
        }
    }

    pub(crate) fn holds_children(self) -> bool {
        matches!(self, Kind::Space | Kind::Folder)
    }

    pub(crate) fn matched_by_url(self) -> bool {
        matches!(self, Kind::Favorite | Kind::Pin)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Up<K> {
    Favorites,
    Spaces,
    Item(K),
}

#[derive(Clone, Debug)]
pub(crate) struct Item<K> {
    pub(crate) key: K,
    pub(crate) kind: Kind,
    pub(crate) title: String,
    pub(crate) url: Option<String>,
    pub(crate) up: Up<K>,
}

#[derive(Debug)]
pub(crate) struct Side<K> {
    pub(crate) items: Vec<Item<K>>,
    at: HashMap<K, usize>,
    children: HashMap<Up<K>, Vec<usize>>,
}

impl<K: Clone + Eq + Hash> Side<K> {
    fn new() -> Side<K> {
        Side {
            items: Vec::new(),
            at: HashMap::new(),
            children: HashMap::new(),
        }
    }

    fn push(&mut self, item: Item<K>) {
        if self.at.contains_key(&item.key) {
            return;
        }
        let index = self.items.len();
        self.at.insert(item.key.clone(), index);
        self.children
            .entry(item.up.clone())
            .or_default()
            .push(index);
        self.items.push(item);
    }

    pub(crate) fn find(&self, key: &K) -> Option<usize> {
        self.at.get(key).copied()
    }

    pub(crate) fn children(&self, up: &Up<K>) -> &[usize] {
        self.children.get(up).map_or(&[], Vec::as_slice)
    }
}

pub(crate) fn remote(
    favorites: Option<&BookmarkNode>,
    spaces: Option<&BookmarkNode>,
) -> Side<BookmarkId> {
    let mut side = Side::new();
    for node in favorites
        .into_iter()
        .flat_map(|folder| &folder.children)
        .filter(|node| !node.is_folder())
    {
        side.push(remote_item(node, Kind::Favorite, Up::Favorites));
    }
    for node in spaces
        .into_iter()
        .flat_map(|folder| &folder.children)
        .filter(|node| node.is_folder())
    {
        side.push(remote_item(node, Kind::Space, Up::Spaces));
        remote_pinned(&mut side, node);
    }
    side
}

fn remote_pinned(side: &mut Side<BookmarkId>, parent: &BookmarkNode) {
    for node in &parent.children {
        let up = Up::Item(parent.id.clone());
        match node.is_folder() {
            true => {
                side.push(remote_item(node, Kind::Folder, up));
                remote_pinned(side, node);
            }
            false => side.push(remote_item(node, Kind::Pin, up)),
        }
    }
}

fn remote_item(node: &BookmarkNode, kind: Kind, up: Up<BookmarkId>) -> Item<BookmarkId> {
    Item {
        key: node.id.clone(),
        kind,
        title: node.title.clone(),
        url: node.url.clone(),
        up,
    }
}

pub(crate) fn local(mirror: &Mirror) -> Side<LinkedItem> {
    let mut side = Side::new();
    for favorite in &mirror.favorites {
        side.push(Item {
            key: LinkedItem::Favorite(favorite.id),
            kind: Kind::Favorite,
            title: favorite.title.clone(),
            url: Some(favorite.url.clone()),
            up: Up::Favorites,
        });
    }
    for space in &mirror.spaces {
        let key = LinkedItem::Space(space.id);
        side.push(Item {
            key,
            kind: Kind::Space,
            title: space.name.clone(),
            url: None,
            up: Up::Spaces,
        });
        local_pinned(&mut side, &space.pinned, key);
    }
    side
}

fn local_pinned(side: &mut Side<LinkedItem>, nodes: &[MirrorNode], parent: LinkedItem) {
    for node in nodes {
        match node {
            MirrorNode::Folder { id, name, children } => {
                let key = LinkedItem::Folder(*id);
                side.push(Item {
                    key,
                    kind: Kind::Folder,
                    title: name.clone(),
                    url: None,
                    up: Up::Item(parent),
                });
                local_pinned(side, children, key);
            }
            MirrorNode::Pin { id, url, title } => side.push(Item {
                key: LinkedItem::Pin(*id),
                kind: Kind::Pin,
                title: title.clone(),
                url: Some(url.clone()),
                up: Up::Item(parent),
            }),
        }
    }
}

pub(crate) struct Base {
    pub(crate) side: Side<BookmarkId>,
    pub(crate) local: HashMap<BookmarkId, LinkedItem>,
}

pub(crate) struct Kept {
    pub(crate) favorites: bool,
    pub(crate) spaces: bool,
}

pub(crate) fn base(sync: Option<&BookmarkSync>, kept: &Kept, remote: &Side<BookmarkId>) -> Base {
    let mut base = Base {
        side: Side::new(),
        local: HashMap::new(),
    };
    let Some(sync) = sync else {
        return base;
    };
    let up = |parent: &str| {
        if parent == sync.roots.favorites {
            Up::Favorites
        } else if parent == sync.roots.spaces {
            Up::Spaces
        } else {
            Up::Item(BookmarkId::from_raw(parent))
        }
    };
    let mut links: Vec<&BookmarkLink> = sync
        .links
        .iter()
        .filter(|link| match Kind::of(link.item) {
            Kind::Favorite => kept.favorites,
            Kind::Space | Kind::Folder | Kind::Pin => kept.spaces,
        })
        .collect();
    let lost = lost_spaces(&links, remote);
    let orphaned = descendants(&links, &lost);
    links.retain(|link| !orphaned.contains(link.bookmark.as_str()));
    links.sort_by_key(|link| link.position);
    for link in links {
        let key = BookmarkId::from_raw(link.bookmark.as_str());
        base.local.insert(key.clone(), link.item);
        base.side.push(Item {
            key,
            kind: Kind::of(link.item),
            title: link.title.clone(),
            url: link.url.clone(),
            up: up(&link.parent),
        });
    }
    base
}

fn lost_spaces<'a>(links: &[&'a BookmarkLink], remote: &Side<BookmarkId>) -> Vec<&'a str> {
    links
        .iter()
        .filter(|link| Kind::of(link.item) == Kind::Space)
        .filter(|link| {
            remote
                .find(&BookmarkId::from_raw(link.bookmark.as_str()))
                .is_none_or(|at| remote.items[at].kind != Kind::Space)
        })
        .map(|link| link.bookmark.as_str())
        .collect()
}

fn descendants<'a>(links: &[&'a BookmarkLink], roots: &[&'a str]) -> HashSet<&'a str> {
    let mut found: HashSet<&str> = HashSet::new();
    let mut pending: Vec<&str> = roots.to_vec();
    while let Some(parent) = pending.pop() {
        for link in links.iter().filter(|link| link.parent == parent) {
            if found.insert(link.bookmark.as_str()) {
                pending.push(link.bookmark.as_str());
            }
        }
    }
    found
}
