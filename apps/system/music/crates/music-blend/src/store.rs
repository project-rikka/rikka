use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use music_core::{Error, Result, Service};
use rikka_worker::Subscription;

use crate::blend::{Blend, BlendId};
use crate::discover::StoreSnapshot;
use crate::folder;
use crate::ledger::{Ledger, Placements};
use crate::watcher::{Outside, Watcher};
use crate::writer::{self, Write, Writer};

const DELETED_FILE: &str = "buried.json";

#[derive(Clone, Debug)]
pub struct Places {
    pub playlists: PathBuf,
    pub state: PathBuf,
}

#[derive(Debug)]
pub struct Store {
    blends: Vec<Arc<Blend>>,
    deleted: BTreeSet<BlendId>,
    writer: Writer,
    watcher: Watcher,
}

impl Store {
    pub fn open(places: Places, services: impl IntoIterator<Item = Service>) -> Result<Store> {
        let services: Vec<Service> = services.into_iter().collect();
        if let Err(error) = fs::create_dir_all(&places.playlists) {
            tracing::warn!(
                "{} does not support playlists: {error}",
                places.playlists.display()
            );
        }
        let mut placements = Placements::new();
        let mut blends: Vec<Arc<Blend>> =
            folder::list(&places.playlists, &services, &Placements::new())
                .map(|listing| listing.blends)
                .unwrap_or_default()
                .into_iter()
                .map(|listed| {
                    placements.insert(listed.blend.id.clone(), listed.placed);
                    Arc::new(listed.blend)
                })
                .collect();
        blends.sort_by_key(|blend| std::cmp::Reverse(blend.created));
        let deleted_at = places.state.join(DELETED_FILE);
        let deleted = fs::read(&deleted_at)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
            .unwrap_or_default()
            .iter()
            .filter_map(|id| BlendId::parse(id))
            .collect();
        let ledger = Ledger::new(placements);
        Ok(Store {
            watcher: Watcher::start(places.playlists.clone(), services, ledger.clone())?,
            writer: Writer::start(
                writer::Places {
                    playlists: places.playlists,
                    deleted: deleted_at,
                },
                ledger,
            )?,
            blends,
            deleted,
        })
    }

    #[must_use]
    pub fn blends(&self) -> &[Arc<Blend>] {
        &self.blends
    }

    #[must_use]
    pub fn find(&self, id: &BlendId) -> Option<&Arc<Blend>> {
        self.blends.iter().find(|blend| &blend.id == id)
    }

    #[must_use]
    pub fn is_deleted(&self, id: &BlendId) -> bool {
        self.deleted.contains(id)
    }

    #[must_use]
    pub fn snapshot(&self) -> StoreSnapshot {
        StoreSnapshot {
            blends: self.blends.clone(),
            deleted: self.deleted.clone(),
        }
    }

    #[must_use]
    pub fn failures(&self) -> Subscription<Error> {
        self.writer.failures()
    }

    #[must_use]
    pub fn outside(&self) -> Subscription<Outside> {
        self.watcher.outside()
    }

    pub fn save(&mut self, blend: Blend) {
        if self.deleted.contains(&blend.id) {
            return;
        }
        let blend = Arc::new(blend);
        self.writer.send(Write::Keep(Arc::clone(&blend)));
        self.upsert(blend);
    }

    pub fn delete(&mut self, id: &BlendId) {
        self.deleted.insert(id.clone());
        self.blends.retain(|blend| &blend.id != id);
        self.writer.send(Write::Listed(self.listed()));
        self.writer.send(Write::Gone(id.clone()));
    }

    pub fn follow(&mut self, outside: Outside) {
        match outside {
            Outside::Edited(blend) => {
                if self.deleted.remove(&blend.id) {
                    self.writer.send(Write::Listed(self.listed()));
                }
                self.upsert(blend);
            }
            Outside::Gone(id) => {
                if self.find(&id).is_some() {
                    self.delete(&id);
                }
            }
        }
    }

    fn upsert(&mut self, blend: Arc<Blend>) {
        match self
            .blends
            .iter_mut()
            .find(|existing| existing.id == blend.id)
        {
            Some(existing) => *existing = blend,
            None => {
                let at = self
                    .blends
                    .partition_point(|existing| existing.created > blend.created);
                self.blends.insert(at, blend);
            }
        }
    }

    fn listed(&self) -> Vec<u8> {
        let listed: Vec<&str> = self.deleted.iter().map(BlendId::as_str).collect();
        serde_json::to_vec_pretty(&listed).expect("blend ids serialize")
    }
}
