use std::collections::{HashMap, HashSet, VecDeque};

use browser_store::LinkedItem;
use rikka_web::BookmarkId;

use crate::sides::{Base, Item, Kind, Side, Up};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Holder {
    Favorites,
    Spaces,
    Entry(usize),
}

#[derive(Clone, Debug)]
pub(crate) struct Present<K> {
    pub(crate) key: K,
    pub(crate) title: String,
    pub(crate) url: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) kind: Kind,
    pub(crate) parent: Option<Holder>,
    pub(crate) title: String,
    pub(crate) url: Option<String>,
    pub(crate) local: Option<Present<LinkedItem>>,
    pub(crate) remote: Option<Present<BookmarkId>>,
    pub(crate) local_up: Option<Holder>,
    pub(crate) remote_up: Option<Holder>,
}

impl Entry {
    pub(crate) fn alive(&self) -> bool {
        self.parent.is_some()
    }
}

#[derive(Debug)]
pub(crate) struct Merged {
    pub(crate) entries: Vec<Entry>,
    order: HashMap<Holder, Vec<usize>>,
}

impl Merged {
    pub(crate) fn order(&self, holder: Holder) -> &[usize] {
        self.order.get(&holder).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn is_dead_with_remote(&self, holder: Option<Holder>) -> bool {
        matches!(holder, Some(Holder::Entry(parent))
            if !self.entries[parent].alive() && self.entries[parent].remote.is_some())
    }

    pub(crate) fn is_dead_with_local(&self, holder: Option<Holder>) -> bool {
        matches!(holder, Some(Holder::Entry(parent))
            if !self.entries[parent].alive() && self.entries[parent].local.is_some())
    }
}

#[derive(Clone, Copy, Debug)]
struct Link {
    kind: Kind,
    local: Option<usize>,
    remote: Option<usize>,
    base: Option<usize>,
}

struct Matching<'a> {
    remote: &'a Side<BookmarkId>,
    local: &'a Side<LinkedItem>,
    base: &'a Base,
    links: Vec<Link>,
    by_remote: Vec<Option<usize>>,
    by_local: Vec<Option<usize>>,
    by_base: Vec<Option<usize>>,
}

pub(crate) fn merge(remote: &Side<BookmarkId>, local: &Side<LinkedItem>, base: &Base) -> Merged {
    let mut matching = Matching {
        remote,
        local,
        base,
        links: Vec::new(),
        by_remote: vec![None; remote.items.len()],
        by_local: vec![None; local.items.len()],
        by_base: vec![None; base.side.items.len()],
    };
    matching.match_base();
    matching.pair();
    matching.leftovers();
    matching.merged()
}

impl<'a> Matching<'a> {
    fn add(
        &mut self,
        kind: Kind,
        local: Option<usize>,
        remote: Option<usize>,
        base: Option<usize>,
    ) -> usize {
        let entry = self.links.len();
        for (side, at) in [
            (&mut self.by_local, local),
            (&mut self.by_remote, remote),
            (&mut self.by_base, base),
        ] {
            if let Some(at) = at {
                side[at] = Some(entry);
            }
        }
        self.links.push(Link {
            kind,
            local,
            remote,
            base,
        });
        entry
    }

    fn match_base(&mut self) {
        let (base, remote, local) = (self.base, self.remote, self.local);
        for (at, item) in base.side.items.iter().enumerate() {
            let remote_at = remote
                .find(&item.key)
                .filter(|&found| remote.items[found].kind == item.kind)
                .filter(|&found| self.by_remote[found].is_none());
            let local_at = base
                .local
                .get(&item.key)
                .and_then(|key| local.find(key))
                .filter(|&found| local.items[found].kind == item.kind)
                .filter(|&found| self.by_local[found].is_none());
            self.add(item.kind, local_at, remote_at, Some(at));
        }
    }

    fn pair(&mut self) {
        let mut queue: VecDeque<Holder> = VecDeque::from([Holder::Favorites, Holder::Spaces]);
        queue.extend(
            self.links
                .iter()
                .enumerate()
                .filter(|(_, link)| {
                    link.kind.holds_children() && link.local.is_some() && link.remote.is_some()
                })
                .map(|(entry, _)| Holder::Entry(entry)),
        );
        while let Some(holder) = queue.pop_front() {
            let mut free: Vec<usize> = self
                .remote_children(holder)
                .iter()
                .copied()
                .filter(|&at| self.by_remote[at].is_none())
                .collect();
            let unpaired: Vec<usize> = self
                .local_children(holder)
                .iter()
                .copied()
                .filter(|&at| self.by_local[at].is_none())
                .collect();
            for local_at in unpaired {
                let item = &self.local.items[local_at];
                let Some(found) = free
                    .iter()
                    .position(|&at| same(&self.remote.items[at], item))
                else {
                    continue;
                };
                let remote_at = free.remove(found);
                let kind = item.kind;
                let entry = self.add(kind, Some(local_at), Some(remote_at), None);
                if kind.holds_children() {
                    queue.push_back(Holder::Entry(entry));
                }
            }
        }
    }

    fn leftovers(&mut self) {
        for at in 0..self.remote.items.len() {
            if self.by_remote[at].is_none() {
                self.add(self.remote.items[at].kind, None, Some(at), None);
            }
        }
        for at in 0..self.local.items.len() {
            if self.by_local[at].is_none() {
                self.add(self.local.items[at].kind, Some(at), None, None);
            }
        }
    }

    fn remote_children(&self, holder: Holder) -> &'a [usize] {
        let remote = self.remote;
        match holder {
            Holder::Favorites => remote.children(&Up::Favorites),
            Holder::Spaces => remote.children(&Up::Spaces),
            Holder::Entry(entry) => self.links[entry].remote.map_or(&[], |at| {
                remote.children(&Up::Item(remote.items[at].key.clone()))
            }),
        }
    }

    fn local_children(&self, holder: Holder) -> &'a [usize] {
        let local = self.local;
        match holder {
            Holder::Favorites => local.children(&Up::Favorites),
            Holder::Spaces => local.children(&Up::Spaces),
            Holder::Entry(entry) => self.links[entry]
                .local
                .map_or(&[], |at| local.children(&Up::Item(local.items[at].key))),
        }
    }

    fn base_children(&self, holder: Holder) -> &'a [usize] {
        let side = &self.base.side;
        match holder {
            Holder::Favorites => side.children(&Up::Favorites),
            Holder::Spaces => side.children(&Up::Spaces),
            Holder::Entry(entry) => self.links[entry].base.map_or(&[], |at| {
                side.children(&Up::Item(side.items[at].key.clone()))
            }),
        }
    }

    fn holder<K: Eq + std::hash::Hash + Clone>(
        side: &Side<K>,
        by: &[Option<usize>],
        up: &Up<K>,
    ) -> Option<Holder> {
        match up {
            Up::Favorites => Some(Holder::Favorites),
            Up::Spaces => Some(Holder::Spaces),
            Up::Item(key) => side.find(key).and_then(|at| by[at]).map(Holder::Entry),
        }
    }

    fn remote_up(&self, entry: usize) -> Option<Holder> {
        let at = self.links[entry].remote?;
        Self::holder(self.remote, &self.by_remote, &self.remote.items[at].up)
    }

    fn local_up(&self, entry: usize) -> Option<Holder> {
        let at = self.links[entry].local?;
        Self::holder(self.local, &self.by_local, &self.local.items[at].up)
    }

    fn base_up(&self, entry: usize) -> Option<Holder> {
        let at = self.links[entry].base?;
        Self::holder(&self.base.side, &self.by_base, &self.base.side.items[at].up)
    }

    fn remote_changed(&self, entry: usize) -> bool {
        let link = self.links[entry];
        let (Some(remote), Some(base)) = (link.remote, link.base) else {
            return false;
        };
        let (remote, base) = (&self.remote.items[remote], &self.base.side.items[base]);
        remote.title != base.title
            || remote.url != base.url
            || self
                .base_up(entry)
                .is_some_and(|up| Some(up) != self.remote_up(entry))
    }

    fn fates(&self) -> Vec<bool> {
        (0..self.links.len())
            .map(|entry| {
                let link = self.links[entry];
                match (link.base, link.local, link.remote) {
                    (Some(_), Some(_), Some(_)) | (None, _, _) => true,
                    (Some(_), None, Some(_)) => self.remote_changed(entry),
                    (Some(_), Some(_), None) => link.kind == Kind::Space,
                    (Some(_), None, None) => false,
                }
            })
            .collect()
    }

    fn settle(&self, alive: &[bool], start: Option<Holder>, remote: bool) -> Option<Holder> {
        let mut holder = start?;
        for _ in 0..=self.links.len() {
            match holder {
                Holder::Entry(entry) if !alive[entry] => {
                    holder = match remote {
                        true => self.remote_up(entry),
                        false => self.local_up(entry),
                    }?;
                }
                settled => return Some(settled),
            }
        }
        None
    }

    fn fits(&self, kind: Kind, holder: Holder) -> bool {
        match (kind, holder) {
            (Kind::Favorite, Holder::Favorites) | (Kind::Space, Holder::Spaces) => true,
            (Kind::Folder | Kind::Pin, Holder::Entry(parent)) => {
                self.links[parent].kind.holds_children()
            }
            _ => false,
        }
    }

    fn parents(&self, alive: &mut [bool], prefer_remote: &[bool]) -> Vec<Option<Holder>> {
        loop {
            let mut parents = vec![None; self.links.len()];
            let mut changed = false;
            for entry in 0..self.links.len() {
                if !alive[entry] {
                    continue;
                }
                let remote = prefer_remote[entry];
                let start = match remote {
                    true => self.remote_up(entry),
                    false => self.local_up(entry),
                };
                match self.settle(alive, start, remote) {
                    Some(holder) if self.fits(self.links[entry].kind, holder) => {
                        parents[entry] = Some(holder);
                    }
                    _ => {
                        alive[entry] = false;
                        changed = true;
                    }
                }
            }
            if !changed {
                return parents;
            }
        }
    }

    fn settled_parents(&self) -> Vec<Option<Holder>> {
        let mut alive = self.fates();
        let mut prefer_remote: Vec<bool> = (0..self.links.len())
            .map(|entry| {
                let moved = matches!(
                    (self.remote_up(entry), self.base_up(entry)),
                    (Some(remote), Some(base)) if remote != base
                );
                moved || self.links[entry].local.is_none()
            })
            .collect();
        loop {
            let parents = self.parents(&mut alive, &prefer_remote);
            let Some(cycle) = cycle(&parents) else {
                return parents;
            };
            match cycle
                .iter()
                .find(|&&entry| !prefer_remote[entry] && self.links[entry].remote.is_some())
            {
                Some(&entry) => prefer_remote[entry] = true,
                None => alive[cycle[0]] = false,
            }
        }
    }

    fn arrange(&self, holder: Holder, members: &[usize]) -> Vec<usize> {
        let set: HashSet<usize> = members.iter().copied().collect();
        let mapped = |children: &[usize], by: &[Option<usize>]| -> Vec<usize> {
            children
                .iter()
                .filter_map(|&at| by[at])
                .filter(|entry| set.contains(entry))
                .collect()
        };
        let remote = mapped(self.remote_children(holder), &self.by_remote);
        let local = mapped(self.local_children(holder), &self.by_local);
        let base = mapped(self.base_children(holder), &self.by_base);
        let common = |sequence: &[usize], other: &[usize]| -> Vec<usize> {
            sequence
                .iter()
                .copied()
                .filter(|entry| other.contains(entry))
                .collect()
        };
        let reordered = common(&remote, &base) != common(&base, &remote);
        let (mut arranged, other) = match reordered {
            true => (remote, local),
            false => (local, remote),
        };
        for (at, entry) in other.iter().enumerate() {
            if arranged.contains(entry) {
                continue;
            }
            let after = other[..at]
                .iter()
                .rev()
                .find_map(|previous| arranged.iter().position(|placed| placed == previous))
                .map_or(0, |found| found + 1);
            arranged.insert(after, *entry);
        }
        for entry in members {
            if !arranged.contains(entry) {
                arranged.push(*entry);
            }
        }
        arranged
    }

    fn merged(self) -> Merged {
        let parents = self.settled_parents();
        let mut members: HashMap<Holder, Vec<usize>> = HashMap::new();
        for (entry, parent) in parents.iter().enumerate() {
            if let Some(parent) = parent {
                members.entry(*parent).or_default().push(entry);
            }
        }
        let order = members
            .iter()
            .map(|(holder, members)| (*holder, self.arrange(*holder, members)))
            .collect();
        let entries = (0..self.links.len())
            .map(|entry| self.entry(entry, parents[entry]))
            .collect();
        Merged { entries, order }
    }

    fn entry(&self, entry: usize, parent: Option<Holder>) -> Entry {
        let link = self.links[entry];
        let remote = link.remote.map(|at| &self.remote.items[at]);
        let local = link.local.map(|at| &self.local.items[at]);
        let base = link.base.map(|at| &self.base.side.items[at]);
        Entry {
            kind: link.kind,
            parent,
            title: pick(
                remote.map(|item| &item.title),
                local.map(|item| &item.title),
                base.map(|item| &item.title),
            ),
            url: pick(
                remote.map(|item| &item.url),
                local.map(|item| &item.url),
                base.map(|item| &item.url),
            ),
            local: local.map(present),
            remote: remote.map(present),
            local_up: self.local_up(entry),
            remote_up: self.remote_up(entry),
        }
    }
}

fn same(remote: &Item<BookmarkId>, local: &Item<LinkedItem>) -> bool {
    remote.kind == local.kind
        && match local.kind.matched_by_url() {
            true => remote.url == local.url,
            false => remote.title == local.title,
        }
}

fn pick<T: PartialEq + Clone + Default>(
    remote: Option<&T>,
    local: Option<&T>,
    base: Option<&T>,
) -> T {
    remote
        .filter(|remote| base != Some(*remote))
        .or(local)
        .or(remote)
        .cloned()
        .unwrap_or_default()
}

fn present<K: Clone>(item: &Item<K>) -> Present<K> {
    Present {
        key: item.key.clone(),
        title: item.title.clone(),
        url: item.url.clone(),
    }
}

fn cycle(parents: &[Option<Holder>]) -> Option<Vec<usize>> {
    for start in 0..parents.len() {
        let mut path = vec![start];
        let mut current = start;
        while let Some(Holder::Entry(parent)) = parents[current] {
            if parent == start {
                return Some(path);
            }
            if path.len() > parents.len() {
                break;
            }
            path.push(parent);
            current = parent;
        }
    }
    None
}
