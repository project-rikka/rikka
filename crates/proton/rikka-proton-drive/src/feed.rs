use std::fmt;
use std::sync::Arc;

use futures_util::future::join_all;
use rikka_proton_account::{EventPage, Feed, FeedError, FeedFuture, Poll};
use rikka_proton_api::offload;

use crate::client::DriveClient;
use crate::error::Error;
use crate::events::EventBatch;
use crate::identifier::{EventId, VolumeId};

type Observer = Arc<dyn Fn(&VolumeId, &EventId, &EventBatch) + Send + Sync>;

#[derive(Clone)]
pub struct DriveFeed {
    drive: DriveClient,
    observer: Option<Observer>,
}

#[derive(Debug)]
pub struct VolumeEvents {
    volume: VolumeId,
    events: EventBatch,
}

impl DriveFeed {
    #[must_use]
    pub fn new(drive: DriveClient) -> DriveFeed {
        DriveFeed {
            drive,
            observer: None,
        }
    }

    #[must_use]
    pub fn observe(
        mut self,
        observer: impl Fn(&VolumeId, &EventId, &EventBatch) + Send + Sync + 'static,
    ) -> DriveFeed {
        self.observer = Some(Arc::new(observer));
        self
    }

    async fn follow_volume(
        &self,
        poll: &Poll<'_, VolumeEvents>,
        volume: &VolumeId,
    ) -> Result<(), FeedError> {
        let drive = &self.drive;
        poll.walk(
            &format!("drive:{volume}"),
            async { Ok(drive.latest_event(volume).await?.as_str().to_owned()) },
            |since| async move { Ok(drive.events(volume, &EventId::from(since.as_str())).await?) },
            |since, events: EventBatch| async move {
                if let Some(observer) = &self.observer {
                    observer(volume, &EventId::from(since.as_str()), &events);
                }
                drive.apply_events(&events);
                if !events.events().is_empty() || events.needs_refresh() {
                    poll.emit(VolumeEvents {
                        volume: volume.clone(),
                        events,
                    });
                }
                Ok(())
            },
        )
        .await
    }
}

impl Feed for DriveFeed {
    type Change = VolumeEvents;

    fn poll<'a>(&'a self, poll: &'a Poll<'a, VolumeEvents>) -> FeedFuture<'a> {
        Box::pin(async move {
            let recalling = self.drive.clone();
            let volumes = match offload(move || recalling.cached_volumes()).await {
                Some(volumes) => volumes,
                None => self.drive.volumes().await?,
            };
            let walks = volumes
                .iter()
                .filter(|volume| volume.is_active())
                .map(|volume| self.follow_volume(poll, volume.id()));
            join_all(walks).await.into_iter().collect()
        })
    }
}

impl fmt::Debug for DriveFeed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DriveFeed")
            .field("drive", &self.drive)
            .field("observed", &self.observer.is_some())
            .finish()
    }
}

impl VolumeEvents {
    #[must_use]
    pub fn volume(&self) -> &VolumeId {
        &self.volume
    }

    #[must_use]
    pub fn events(&self) -> &EventBatch {
        &self.events
    }
}

impl EventPage for EventBatch {
    fn next_id(&self) -> &str {
        self.id().as_str()
    }

    fn has_more(&self) -> bool {
        self.has_more()
    }
}

impl From<Error> for FeedError {
    fn from(error: Error) -> FeedError {
        FeedError::new(error, Error::api)
    }
}
