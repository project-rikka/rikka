use futures_util::future::try_join_all;
use rikka_proton_cache::{Cache, SINGLETON_ID, Table};
use serde::{Deserialize, Serialize};

use crate::api::{ItemRevision, ShareEventList, ShareKeyResponse, ShareResponse};
use crate::client::PassClient;
use crate::error::Result;
use crate::identifier::{EventId, ShareId};
use crate::pass_item::PassItem;
use crate::share::Share;

const SHARES: Table<Vec<ShareResponse>> = Table::new("pass.shares");
const SHARE_KEYS: Table<Vec<ShareKeyResponse>> = Table::new("pass.share-keys");
const ITEMS: Table<CachedItems> = Table::new("pass.items");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedItems {
    revisions: Vec<ItemRevision>,
    event: Option<EventId>,
}

impl PassClient {
    #[must_use]
    pub fn with_cache(mut self, cache: Cache) -> PassClient {
        self.account = self.account.with_cache(cache);
        self
    }

    #[must_use]
    pub fn cache(&self) -> Option<&Cache> {
        self.account.cache()
    }

    pub async fn cached_shares(&self) -> Result<Option<Vec<Share>>> {
        let Some(responses) = self.stored_shares() else {
            return Ok(None);
        };
        let decryptions = responses
            .into_iter()
            .map(|response| self.decrypt_share(response));
        Ok(Some(try_join_all(decryptions).await?))
    }

    pub async fn cached_items(&self, share: &ShareId) -> Result<Option<Vec<PassItem>>> {
        let Some(cached) = self.stored_items(share) else {
            return Ok(None);
        };
        Ok(Some(self.decrypt_items(share, cached.revisions).await?))
    }

    pub async fn sync_items(&self, share: &ShareId) -> Result<Vec<PassItem>> {
        let revisions = match self.stored_items(share) {
            Some(CachedItems {
                revisions,
                event: Some(event),
            }) => match self.replay_events(share, revisions, event).await {
                Ok(revisions) => revisions,
                Err(error) => {
                    tracing::debug!(%error, "failed to replay item events, relisting");
                    self.fetch_all_revisions(share).await?
                }
            },
            _ => self.fetch_all_revisions(share).await?,
        };

        self.decrypt_items(share, revisions).await
    }

    pub(crate) fn stored_shares(&self) -> Option<Vec<ShareResponse>> {
        self.cache()?.get(&SHARES, SINGLETON_ID)
    }

    pub(crate) fn cache_shares(&self, shares: &[ShareResponse]) {
        if let Some(cache) = self.cache() {
            cache.put(&SHARES, SINGLETON_ID, shares.to_vec());
        }
    }

    pub(crate) fn cache_share(&self, share: &ShareResponse) {
        let Some(mut cached) = self.stored_shares() else {
            return;
        };
        match cached
            .iter_mut()
            .find(|cached| cached.share_id() == share.share_id())
        {
            Some(cached) => *cached = share.clone(),
            None => cached.push(share.clone()),
        }
        self.cache_shares(&cached);
    }

    pub(crate) fn stored_share_keys(
        &self,
        share: &ShareId,
        rotations: &[i64],
    ) -> Option<Vec<ShareKeyResponse>> {
        if rotations.is_empty() {
            return None;
        }
        let cached = self.cache()?.get(&SHARE_KEYS, share.as_str())?;
        rotations
            .iter()
            .all(|rotation| cached.iter().any(|key| key.rotation() == *rotation))
            .then_some(cached)
    }

    pub(crate) fn cache_share_keys(&self, share: &ShareId, keys: &[ShareKeyResponse]) {
        if let Some(cache) = self.cache() {
            cache.put(&SHARE_KEYS, share.as_str(), keys.to_vec());
        }
    }

    pub(crate) fn cache_items(
        &self,
        share: &ShareId,
        revisions: &[ItemRevision],
        event: Option<EventId>,
    ) {
        if let Some(cache) = self.cache() {
            cache.put(
                &ITEMS,
                share.as_str(),
                CachedItems {
                    revisions: revisions.to_vec(),
                    event,
                },
            );
        }
    }

    pub(crate) fn apply_events(&self, share: &ShareId, since: &EventId, events: &ShareEventList) {
        if let Some(updated) = events.updated_share() {
            self.cache_share(updated);
        }
        let Some(cache) = self.cache() else {
            return;
        };
        let Some(mut cached) = cache.get(&ITEMS, share.as_str()) else {
            return;
        };
        if cached.event.as_ref() != Some(since) {
            return;
        }
        apply_to_revisions(&mut cached.revisions, events);
        cached.event = Some(events.latest_event().clone());
        cache.put(&ITEMS, share.as_str(), cached);
    }

    pub(crate) fn remove_cached_share(&self, share: &ShareId) {
        let Some(cache) = self.cache() else {
            return;
        };
        if let Some(mut cached) = self.stored_shares() {
            cached.retain(|cached| cached.share_id() != share);
            cache.put(&SHARES, SINGLETON_ID, cached);
        }
        let mut batch = cache.batch();
        batch
            .remove(&SHARE_KEYS, share.as_str())
            .remove(&ITEMS, share.as_str());
        batch.commit();
    }

    fn stored_items(&self, share: &ShareId) -> Option<CachedItems> {
        self.cache()?.get(&ITEMS, share.as_str())
    }

    async fn fetch_all_revisions(&self, share: &ShareId) -> Result<Vec<ItemRevision>> {
        let latest = self.latest_event(share).await?;
        let revisions = self.share_revisions(share).await?;
        self.cache_items(share, &revisions, Some(latest));
        Ok(revisions)
    }

    async fn replay_events(
        &self,
        share: &ShareId,
        mut revisions: Vec<ItemRevision>,
        mut event: EventId,
    ) -> Result<Vec<ItemRevision>> {
        loop {
            let events = self.share_event_list(share, &event).await?;
            if let Some(rotation) = events.new_key_rotation() {
                self.share_keys_for_rotations(share, &[rotation]).await?;
            }
            if let Some(updated) = events.updated_share() {
                self.cache_share(updated);
            }
            apply_to_revisions(&mut revisions, &events);
            event = events.latest_event().clone();
            if !events.is_pending() {
                break;
            }
        }

        self.cache_items(share, &revisions, Some(event));
        Ok(revisions)
    }
}

fn apply_to_revisions(revisions: &mut Vec<ItemRevision>, events: &ShareEventList) {
    revisions.retain(|revision| !events.deleted_items().contains(revision.item_id()));
    for updated in events.updated_items() {
        match revisions
            .iter_mut()
            .find(|cached| cached.item_id() == updated.item_id())
        {
            Some(cached) => *cached = updated.clone(),
            None => revisions.push(updated.clone()),
        }
    }
}
