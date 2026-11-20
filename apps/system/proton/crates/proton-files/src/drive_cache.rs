use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use rikka_proton_cache::{Cache, SINGLETON_ID, Slot, Staged, Table};
use rikka_proton_drive::{LinkId, ShareId, ShareKeys, VolumeId};
use serde::{Deserialize, Serialize};

use crate::entry::Entry;

const ROOT: Table<CachedRoot> = Table::new("app.drive.root");
const ENTRIES: Table<Vec<Entry>> = Table::new("app.drive.entries");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CachedRoot {
    pub(crate) volume: VolumeId,
    pub(crate) share: ShareId,
    pub(crate) root: LinkId,
}

#[derive(Clone, Debug)]
pub struct DriveCache {
    cache: Cache,
    root: Slot<CachedRoot>,
    entries: Arc<Mutex<HashMap<LinkId, Slot<Vec<Entry>>>>>,
}

impl DriveCache {
    #[must_use]
    pub fn new(cache: Cache) -> DriveCache {
        DriveCache {
            root: cache.slot(&ROOT, SINGLETON_ID),
            cache,
            entries: Arc::default(),
        }
    }

    #[must_use]
    pub fn cached_root_listing(&self) -> Option<(LinkId, Vec<Entry>)> {
        let cached = self.cached_root()?;
        let entries = self.cached_entries(&cached.root)?;

        Some((cached.root, entries))
    }

    #[must_use]
    pub fn cached_entries(&self, at: &LinkId) -> Option<Vec<Entry>> {
        self.cache.get(&ENTRIES, at.as_str())
    }

    pub(crate) fn cached_root(&self) -> Option<CachedRoot> {
        self.root.read()
    }

    pub(crate) fn stage_root(&self, share: &ShareKeys) -> Staged<CachedRoot> {
        self.root.stage(CachedRoot {
            volume: share.volume_id().clone(),
            share: share.id().clone(),
            root: share.root_id().clone(),
        })
    }

    pub(crate) fn stage_entries(&self, at: &LinkId, entries: Vec<Entry>) -> Staged<Vec<Entry>> {
        self.entries
            .lock()
            .entry(at.clone())
            .or_insert_with(|| self.cache.slot(&ENTRIES, at.as_str()))
            .stage(entries)
    }
}
