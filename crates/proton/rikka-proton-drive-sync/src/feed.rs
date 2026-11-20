use std::sync::Arc;

use rikka_proton_drive::{DriveClient, DriveFeed, EventBatch, EventId, VolumeId};
use tokio::sync::broadcast;

const FEED_CAPACITY: usize = 64;

#[derive(Debug, Clone)]
pub struct VolumeFeed {
    sender: broadcast::Sender<FeedBatch>,
}

#[derive(Debug, Clone)]
pub(crate) struct FeedBatch {
    pub(crate) volume: VolumeId,
    pub(crate) since: EventId,
    pub(crate) batch: Arc<EventBatch>,
}

impl VolumeFeed {
    #[must_use]
    pub fn new() -> VolumeFeed {
        VolumeFeed {
            sender: broadcast::Sender::new(FEED_CAPACITY),
        }
    }

    #[must_use]
    pub fn follow(&self, drive: DriveClient) -> DriveFeed {
        let feed = self.clone();
        DriveFeed::new(drive)
            .observe(move |volume, since, batch| feed.publish(volume, since, batch))
    }

    pub fn publish(&self, volume: &VolumeId, since: &EventId, batch: &EventBatch) {
        if self.sender.receiver_count() == 0 {
            return;
        }
        self.sender
            .send(FeedBatch {
                volume: volume.clone(),
                since: since.clone(),
                batch: Arc::new(batch.clone()),
            })
            .ok();
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<FeedBatch> {
        self.sender.subscribe()
    }
}

impl Default for VolumeFeed {
    fn default() -> VolumeFeed {
        VolumeFeed::new()
    }
}
