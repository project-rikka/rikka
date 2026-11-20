use rikka_proton_cache::{BlobArea, Cache, SINGLETON_ID, Table, Wired};
use serde::{Deserialize, Serialize};

use crate::client::DriveClient;
use crate::error::Result;
use crate::events::{EventAction, EventBatch, EventLog};
use crate::folder::Folder;
use crate::identifier::{LinkId, ShareId, ThumbnailId, VolumeId};
use crate::link::Link;
use crate::node::Node;
use crate::share::Share;
use crate::volume::Volume;

const VOLUMES: Table<Vec<Wired<Volume>>> = Table::new("drive.volumes");
const SHARES: Table<Wired<Share>> = Table::new("drive.shares");
const LINKS: Table<CachedLink> = Table::new("drive.links");
const CHILDREN: Table<Vec<LinkId>> = Table::new("drive.children");
const THUMBNAILS: BlobArea = BlobArea::new("drive.thumbnails", 64 * 1024 * 1024);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedLink {
    volume: VolumeId,
    link: Wired<Link>,
}

impl DriveClient {
    #[must_use]
    pub fn with_cache(mut self, cache: Cache) -> DriveClient {
        self.account = self.account.with_cache(cache);
        self
    }

    #[must_use]
    pub fn cache(&self) -> Option<&Cache> {
        self.account.cache()
    }

    #[must_use]
    pub fn cached_volumes(&self) -> Option<Vec<Volume>> {
        let cached = self.cache()?.get(&VOLUMES, SINGLETON_ID)?;
        Some(cached.into_iter().map(Wired::into_value).collect())
    }

    #[must_use]
    pub fn cached_share(&self, id: &ShareId) -> Option<Share> {
        self.cache()?
            .get(&SHARES, id.as_str())
            .map(Wired::into_value)
    }

    #[must_use]
    pub fn cached_link(&self, id: &LinkId) -> Option<Link> {
        let cached = self.cache()?.get(&LINKS, id.as_str())?;
        Some(cached.link.into_value().in_volume(&cached.volume))
    }

    #[must_use]
    pub fn cached_child_ids(&self, folder: &LinkId) -> Option<Vec<LinkId>> {
        self.cache()?.get(&CHILDREN, folder.as_str())
    }

    pub async fn cached_children(&self, folder: &Folder) -> Result<Option<Vec<Node>>> {
        let Some(ids) = self.cached_child_ids(folder.id()) else {
            return Ok(None);
        };
        let Some(links) = ids
            .iter()
            .map(|id| self.cached_link(id))
            .collect::<Option<Vec<Link>>>()
        else {
            return Ok(None);
        };
        let listed = links
            .into_iter()
            .filter(crate::listing::is_listed)
            .collect();
        Ok(Some(
            self.decrypt_links_with_key(listed, folder.keys().key().clone())
                .await?,
        ))
    }

    pub fn apply_events(&self, batch: &EventBatch) {
        self.invalidate_for_events(batch.needs_refresh(), batch.events());
    }

    pub fn apply_event_log(&self, log: &EventLog) {
        self.invalidate_for_events(log.needs_refresh(), log.events());
    }

    pub fn forget_cached(&self) {
        if let Some(cache) = self.cache() {
            let mut batch = cache.batch();
            batch
                .clear(&VOLUMES)
                .clear(&SHARES)
                .clear(&LINKS)
                .clear(&CHILDREN);
            batch.commit();
        }
    }

    pub(crate) fn cache_volumes(&self, volumes: &[Wired<Volume>]) {
        if let Some(cache) = self.cache() {
            cache.put(&VOLUMES, SINGLETON_ID, volumes.to_vec());
        }
    }

    pub(crate) fn cache_share(&self, share: &Wired<Share>) {
        if let Some(cache) = self.cache() {
            cache.put(&SHARES, share.id().as_str(), share.clone());
        }
    }

    pub(crate) fn cache_links(&self, volume: &VolumeId, links: &[Wired<Link>]) {
        let Some(cache) = self.cache() else {
            return;
        };
        let mut batch = cache.batch();
        for link in links {
            batch.put(
                &LINKS,
                link.id().as_str(),
                CachedLink {
                    volume: volume.clone(),
                    link: link.clone(),
                },
            );
        }
        batch.commit();
    }

    pub(crate) fn cache_children(&self, folder: &LinkId, children: &[LinkId]) {
        if let Some(cache) = self.cache() {
            cache.put(&CHILDREN, folder.as_str(), children.to_vec());
        }
    }

    pub(crate) fn stored_thumbnail(&self, id: &ThumbnailId) -> Option<Vec<u8>> {
        self.cache()?.blob(&THUMBNAILS, id.as_str())
    }

    pub(crate) fn cache_thumbnail(&self, id: &ThumbnailId, sealed: &[u8]) {
        if let Some(cache) = self.cache() {
            cache.put_blob(&THUMBNAILS, id.as_str(), sealed);
        }
    }

    fn invalidate_for_events(&self, refresh: bool, events: &[crate::events::DriveEvent]) {
        let Some(cache) = self.cache() else {
            return;
        };
        if refresh {
            self.forget_cached();
            return;
        }

        let mut batch = cache.batch();
        for event in events {
            let link = event.link();
            let moves_link = event.action() != EventAction::UpdateMetadata;
            if moves_link {
                if let Some(cached) = cache.get(&LINKS, link.id().as_str())
                    && let Some(parent) = cached.link.parent_id()
                {
                    batch.remove(&CHILDREN, parent.as_str());
                }
                if let Some(parent) = link.parent_id() {
                    batch.remove(&CHILDREN, parent.as_str());
                }
            }
            if event.action() == EventAction::Delete {
                batch.remove(&CHILDREN, link.id().as_str());
            }
            batch.remove(&LINKS, link.id().as_str());
        }
        batch.commit();
    }
}
