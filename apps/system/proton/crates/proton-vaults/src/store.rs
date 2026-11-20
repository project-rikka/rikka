use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use rikka_proton_pass::{ItemId, Share, ShareId, ShareTarget, VaultColor, VaultIcon};

use crate::decrypted_item::DecryptedItem;
use crate::entry::Entry;
use crate::opened::OpenedItem;
use crate::options::SortKey;

#[derive(Clone, Debug)]
pub struct Vault {
    pub share: ShareId,
    pub name: String,
    pub description: String,
    pub icon: i32,
    pub color: i32,
}

impl Vault {
    #[must_use]
    pub fn from_share(share: &Share) -> Option<Vault> {
        if share.target() != ShareTarget::Vault {
            return None;
        }
        let vault = share.vault();

        Some(Vault {
            share: share.id().clone(),
            name: vault.map_or("", |vault| vault.name.as_str()).to_owned(),
            description: vault
                .and_then(|vault| vault.description())
                .unwrap_or_default()
                .to_owned(),
            icon: vault.map_or(VaultIcon::Unspecified as i32, |vault| vault.icon() as i32),
            color: vault.map_or(VaultColor::Unspecified as i32, |vault| vault.color() as i32),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemScope {
    #[default]
    All,
    Trash,
}

#[derive(Default)]
pub struct Vaults {
    vaults: Vec<Vault>,
    listed: HashMap<ShareId, Listing>,
    ordered: Vec<Indexed>,
    shown: Arc<Vec<Arc<Entry>>>,
    counts: HashMap<ShareId, usize>,
    all: usize,
    trashed: usize,
    chosen: Option<ShareId>,
    scope: ItemScope,
    sorted: SortKey,
    keyword: String,
}

#[derive(Default)]
struct Listing {
    entries: HashMap<ItemId, Indexed>,
    details: HashMap<ItemId, OpenedItem>,
}

impl Listing {
    fn from_items(items: Vec<DecryptedItem>) -> Listing {
        let mut listing = Listing::default();
        for unlocked in items {
            listing.put(unlocked);
        }
        listing
    }

    fn put(&mut self, unlocked: DecryptedItem) {
        let DecryptedItem { entry, opened } = unlocked;
        self.details.insert(entry.id.clone(), opened);
        self.entries
            .insert(entry.id.clone(), Indexed::from_entry(entry));
    }

    fn remove(&mut self, id: &ItemId) {
        self.entries.remove(id);
        self.details.remove(id);
    }
}

#[derive(Clone)]
struct Indexed {
    entry: Arc<Entry>,
    folded: Arc<str>,
}

impl Indexed {
    fn from_entry(entry: Entry) -> Indexed {
        Indexed {
            folded: entry.folded().into(),
            entry: Arc::new(entry),
        }
    }
}

impl Vaults {
    #[must_use]
    pub fn new() -> Vaults {
        Vaults::default()
    }

    #[must_use]
    pub fn vaults(&self) -> &[Vault] {
        &self.vaults
    }

    #[must_use]
    pub fn vault(&self, share: &ShareId) -> Option<&Vault> {
        self.vaults.iter().find(|vault| &vault.share == share)
    }

    #[must_use]
    pub fn selected(&self) -> Option<&ShareId> {
        self.chosen.as_ref()
    }

    #[must_use]
    pub fn scope(&self) -> ItemScope {
        self.scope
    }

    #[must_use]
    pub fn sorted(&self) -> SortKey {
        self.sorted
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.vaults.is_empty()
    }

    pub fn restore(&mut self, vaults: Vec<Vault>, lists: Vec<(ShareId, Vec<DecryptedItem>)>) {
        for (share, items) in lists {
            self.listed.insert(share, Listing::from_items(items));
        }
        self.set_vaults(vaults);
    }

    pub fn set_vaults(&mut self, vaults: Vec<Vault>) {
        self.listed
            .retain(|share, _| vaults.iter().any(|vault| &vault.share == share));
        if let Some(chosen) = &self.chosen
            && !vaults.iter().any(|vault| &vault.share == chosen)
        {
            self.chosen = None;
        }
        self.vaults = vaults;
        self.gathered();
    }

    pub fn synced(&mut self, share: &ShareId, items: Vec<DecryptedItem>) {
        self.listed
            .insert(share.clone(), Listing::from_items(items));
        self.gathered();
    }

    pub fn select(&mut self, share: Option<ShareId>) {
        self.chosen = share;
        self.scope = ItemScope::All;
        self.settle();
    }

    pub fn show(&mut self, scope: ItemScope) {
        self.scope = scope;
        self.settle();
    }

    pub fn sort(&mut self, sorted: SortKey) {
        self.sorted = sorted;
        self.order();
        self.settle();
    }

    pub fn set_filter(&mut self, keyword: impl AsRef<str>) {
        self.keyword = keyword.as_ref().trim().to_lowercase();
        self.settle();
    }

    #[must_use]
    pub fn count(&self, share: &ShareId) -> usize {
        self.counts.get(share).copied().unwrap_or_default()
    }

    #[must_use]
    pub fn trashed(&self) -> usize {
        self.trashed
    }

    #[must_use]
    pub fn all(&self) -> usize {
        self.all
    }

    #[must_use]
    pub fn shown(&self) -> &Arc<Vec<Arc<Entry>>> {
        &self.shown
    }

    #[must_use]
    pub fn entry(&self, share: &ShareId, id: &ItemId) -> Option<&Entry> {
        self.listed
            .get(share)?
            .entries
            .get(id)
            .map(|indexed| indexed.entry.as_ref())
    }

    #[must_use]
    pub fn listed(&self, share: &ShareId) -> Vec<Arc<Entry>> {
        self.listed.get(share).map_or_else(Vec::new, |listing| {
            listing
                .entries
                .values()
                .map(|indexed| Arc::clone(&indexed.entry))
                .collect()
        })
    }

    #[must_use]
    pub fn opened(&self, share: &ShareId, id: &ItemId) -> Option<OpenedItem> {
        let listing = self.listed.get(share)?;
        let entry = &listing.entries.get(id)?.entry;
        let mut opened = listing.details.get(id)?.clone();
        opened.pinned = entry.pinned;
        opened.trashed = entry.trashed;
        Some(opened)
    }

    pub fn put(&mut self, unlocked: DecryptedItem) {
        self.listed
            .entry(unlocked.entry.share.clone())
            .or_default()
            .put(unlocked);
        self.gathered();
    }

    pub fn remove(&mut self, share: &ShareId, ids: &[ItemId]) {
        if let Some(listing) = self.listed.get_mut(share) {
            for id in ids {
                listing.remove(id);
            }
        }
        self.gathered();
    }

    pub fn set_trashed(&mut self, share: &ShareId, ids: &[ItemId], trashed: bool) {
        for id in ids {
            self.amend(share, id, |entry| entry.trashed = trashed);
        }
        self.gathered();
    }

    pub fn set_pinned(&mut self, share: &ShareId, id: &ItemId, pinned: bool) {
        self.amend(share, id, |entry| entry.pinned = pinned);
        self.gathered();
    }

    fn amend(&mut self, share: &ShareId, id: &ItemId, change: impl FnOnce(&mut Entry)) {
        if let Some(indexed) = self
            .listed
            .get_mut(share)
            .and_then(|listing| listing.entries.get_mut(id))
        {
            change(Arc::make_mut(&mut indexed.entry));
        }
    }

    fn gathered(&mut self) {
        self.counts = self
            .listed
            .iter()
            .map(|(share, listing)| {
                (
                    share.clone(),
                    listing
                        .entries
                        .values()
                        .filter(|indexed| !indexed.entry.trashed)
                        .count(),
                )
            })
            .collect();
        self.all = self.counts.values().sum();
        self.trashed = self
            .listed
            .values()
            .flat_map(|listing| listing.entries.values())
            .filter(|indexed| indexed.entry.trashed)
            .count();
        self.order();
        self.settle();
    }

    fn order(&mut self) {
        let sorted = self.sorted;
        let mut ordered: Vec<Indexed> = self
            .listed
            .values()
            .flat_map(|listing| listing.entries.values().cloned())
            .collect();
        ordered.sort_by(|one, other| {
            let (one, other) = (&one.entry, &other.entry);
            other
                .pinned
                .cmp(&one.pinned)
                .then_with(|| sorted.compare(one, other))
                .then_with(|| one.id.cmp(&other.id))
        });
        self.ordered = ordered;
    }

    fn settle(&mut self) {
        let trash = self.scope == ItemScope::Trash;
        let keyword = self.keyword.as_str();
        let chosen = self.chosen.as_ref();

        let shown: Vec<Arc<Entry>> = self
            .ordered
            .iter()
            .filter(|indexed| {
                let entry = &indexed.entry;
                entry.trashed == trash
                    && chosen.is_none_or(|share| &entry.share == share)
                    && (keyword.is_empty() || indexed.folded.contains(keyword))
            })
            .map(|indexed| Arc::clone(&indexed.entry))
            .collect();
        self.shown = Arc::new(shown);
    }
}

impl fmt::Debug for Vaults {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Vaults")
            .field("vaults", &self.vaults.len())
            .field("listed", &self.listed)
            .field("shown", &self.shown.len())
            .field("chosen", &self.chosen)
            .field("scope", &self.scope)
            .field("sorted", &self.sorted)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Listing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Listing")
            .field("entries", &self.entries.len())
            .field("details", &self.details.len())
            .finish()
    }
}
