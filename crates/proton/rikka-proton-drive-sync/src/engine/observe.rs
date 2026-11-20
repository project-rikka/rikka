use std::collections::{BTreeSet, HashMap};

use rikka_proton_drive::{DriveEvent, EventId, VolumeId};
use rikka_proton_drive_sync_state::{Changes, RemoteItem};
use tokio::sync::broadcast;
use tokio::time::Instant;

use crate::engine::{Engine, FEED_FRESHNESS, Scope};
use crate::error::Result;
use crate::feed::FeedBatch;
use crate::remote::events::{RemoteUpdate, apply_events};
use crate::setup::enumerated_key;

struct EventPage<'a> {
    refresh: bool,
    events: &'a [DriveEvent],
    last_event: EventId,
}

impl Engine {
    pub(super) async fn observe_remote(&mut self, scope: Scope) {
        let now = Instant::now();
        let volumes: BTreeSet<String> = self
            .roots
            .iter()
            .filter(|root| root.remote.is_some())
            .map(|root| root.record.volume.clone())
            .filter(|volume| scope == Scope::All || !self.recently_fed(volume, now))
            .collect();
        if volumes.is_empty() {
            return;
        }
        let mut failure = None;
        for volume in volumes {
            if let Err(error) = self.observe_volume(&VolumeId::from(volume)).await {
                tracing::warn!(%error, "failed to fetch remote changes");
                failure = Some(error.to_string());
            }
        }
        self.offline = failure;
    }

    pub(super) async fn apply_feed_batch(&mut self, feed: &FeedBatch) {
        let on_volume = self.roots_on_volume(&feed.volume);
        if on_volume.is_empty() {
            return;
        }
        match self.apply_feed(feed, &on_volume).await {
            Ok(true) => {
                self.fed_volumes.insert(feed.volume.clone(), Instant::now());
                self.offline = None;
            }
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(%error, "failed to apply volume feed changes");
                self.offline = Some(error.to_string());
            }
        }
    }

    fn recently_fed(&self, volume: &str, now: Instant) -> bool {
        self.fed_volumes
            .get(&VolumeId::from(volume))
            .is_some_and(|fed_at| now.duration_since(*fed_at) < FEED_FRESHNESS)
    }

    fn roots_on_volume(&self, volume: &VolumeId) -> Vec<usize> {
        (0..self.roots.len())
            .filter(|index| {
                let root = &self.roots[*index];
                root.remote.is_some() && root.record.volume == volume.as_str()
            })
            .collect()
    }

    async fn apply_feed(&mut self, feed: &FeedBatch, on_volume: &[usize]) -> Result<bool> {
        let cursor = self.cursor(&feed.volume).await?;
        if cursor.as_ref() == Some(feed.batch.id()) {
            return Ok(true);
        }
        let continues = cursor.as_ref() == Some(&feed.since)
            && on_volume.iter().all(|index| self.roots[*index].enumerated);
        if !continues {
            return Ok(false);
        }
        self.apply_remote_events(
            &feed.volume,
            on_volume,
            EventPage {
                refresh: feed.batch.needs_refresh(),
                events: feed.batch.events(),
                last_event: feed.batch.id().clone(),
            },
        )
        .await?;
        Ok(true)
    }

    async fn cursor(&mut self, volume: &VolumeId) -> Result<Option<EventId>> {
        if let Some(cursor) = self.cursors.get(volume) {
            return Ok(Some(cursor.clone()));
        }
        let volume_id = volume.as_str().to_owned();
        let stored = self
            .persist
            .run(move |store| store.cursor(&volume_id))
            .await?
            .map(EventId::from);
        if let Some(cursor) = &stored {
            self.cursors.insert(volume.clone(), cursor.clone());
        }
        Ok(stored)
    }

    async fn observe_volume(&mut self, volume: &VolumeId) -> Result<()> {
        let on_volume = self.roots_on_volume(volume);
        let cursor = match self.cursor(volume).await? {
            Some(cursor) => cursor,
            None => {
                let latest = self.client.latest_event(volume).await?;
                for index in &on_volume {
                    self.roots[*index].enumerated = false;
                }
                latest
            }
        };
        for index in &on_volume {
            if !self.roots[*index].enumerated {
                self.enumerate(*index).await?;
            }
        }

        let log = self.client.catch_up(volume, &cursor).await?;
        self.apply_remote_events(
            volume,
            &on_volume,
            EventPage {
                refresh: log.needs_refresh(),
                events: log.events(),
                last_event: log.last().clone(),
            },
        )
        .await
    }

    async fn apply_remote_events(
        &mut self,
        volume: &VolumeId,
        on_volume: &[usize],
        page: EventPage<'_>,
    ) -> Result<()> {
        if page.refresh {
            for index in on_volume {
                self.enumerate(*index).await?;
            }
        } else if !page.events.is_empty() {
            for index in on_volume {
                let root = &mut self.roots[*index];
                let Some(remote) = root.remote.as_mut() else {
                    continue;
                };
                match apply_events(remote, &self.client, &root.state, page.events).await? {
                    RemoteUpdate::Changes(changes) => self.commit(*index, changes).await?,
                    RemoteUpdate::RootRemoved => {
                        tracing::warn!(root = %root.path().display(), "remote sync root removed");
                        root.remote = None;
                    }
                }
            }
        }

        let (volume_text, event_text) = (
            volume.as_str().to_owned(),
            page.last_event.as_str().to_owned(),
        );
        self.persist
            .run(move |store| store.set_cursor(&volume_text, &event_text))
            .await?;
        self.cursors.insert(volume.clone(), page.last_event);
        Ok(())
    }

    async fn enumerate(&mut self, index: usize) -> Result<()> {
        let root = &mut self.roots[index];
        let Some(remote) = root.remote.as_mut() else {
            return Ok(());
        };
        let found = remote.enumerate(&self.client).await?;
        let found: HashMap<String, RemoteItem> = found
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect();
        let mut changes = Changes::default();
        for (id, item) in &found {
            if root.state.remote().get(id) != Some(item) {
                changes.remote.push(item.clone());
            }
        }
        changes.remote_removed = root
            .state
            .remote()
            .keys()
            .filter(|id| !found.contains_key(*id))
            .cloned()
            .collect();
        let id = root.id;
        self.commit(index, changes).await?;
        let key = enumerated_key(id);
        self.persist
            .run(move |store| store.set_value(&key, "1"))
            .await?;
        self.roots[index].enumerated = true;
        Ok(())
    }
}

pub(super) async fn next_feed_batch(
    feed: &mut Option<broadcast::Receiver<FeedBatch>>,
) -> std::result::Result<FeedBatch, broadcast::error::RecvError> {
    match feed {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}
