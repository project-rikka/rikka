use std::collections::HashMap;
use std::sync::Weak;
use std::sync::atomic::{AtomicU64, Ordering};

use discord_core::VoicePacket;
use futures_channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use rikka_sync::Mutex;
use rikka_worker::EventNotifier;

use crate::settings::volume;
use crate::voice::shared::Shared;

pub(crate) struct Attached {
    minted: AtomicU64,
    arriving: Mutex<Vec<(u64, UnboundedReceiver<VoicePacket>)>>,
    volumes: Mutex<HashMap<u64, f32>>,
    revision: AtomicU64,
    bell: EventNotifier,
}

impl Attached {
    pub(crate) fn new(bell: EventNotifier) -> Attached {
        Attached {
            minted: AtomicU64::new(0),
            arriving: Mutex::new(Vec::new()),
            volumes: Mutex::new(HashMap::new()),
            revision: AtomicU64::new(0),
            bell,
        }
    }

    fn ring(&self) {
        if let Err(error) = self.bell.notify() {
            tracing::warn!(%error, "failed to wake the voice receiver");
        }
    }

    pub(crate) fn attach(&self) -> (u64, UnboundedSender<VoicePacket>) {
        let id = self.minted.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::unbounded();
        self.arriving.lock().push((id, receiver));
        self.ring();
        (id, sender)
    }

    pub(crate) fn arrived(&self) -> Vec<(u64, UnboundedReceiver<VoicePacket>)> {
        std::mem::take(&mut *self.arriving.lock())
    }

    pub(crate) fn set_volume(&self, id: u64, level: f32) {
        self.volumes.lock().insert(id, volume(level));
        self.revision.fetch_add(1, Ordering::Release);
        self.ring();
    }

    pub(crate) fn forget(&self, id: u64) {
        self.volumes.lock().remove(&id);
    }

    pub(crate) fn volumes_since(&self, known: u64) -> Option<(u64, HashMap<u64, f32>)> {
        let revision = self.revision.load(Ordering::Acquire);
        (revision != known).then(|| (revision, self.volumes.lock().clone()))
    }
}

pub struct StreamSound {
    id: u64,
    packets: UnboundedSender<VoicePacket>,
    shared: Weak<Shared>,
}

impl StreamSound {
    pub(crate) fn new(
        id: u64,
        packets: UnboundedSender<VoicePacket>,
        shared: Weak<Shared>,
    ) -> StreamSound {
        StreamSound {
            id,
            packets,
            shared,
        }
    }

    pub fn packet(&self, packet: VoicePacket) {
        let _ = self.packets.unbounded_send(packet);
    }

    pub fn set_volume(&self, level: f32) {
        if let Some(shared) = self.shared.upgrade() {
            shared.attached().set_volume(self.id, level);
        }
    }
}

impl Drop for StreamSound {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.upgrade() {
            shared.attached().forget(self.id);
        }
    }
}

impl std::fmt::Debug for Attached {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Attached")
            .field("revision", &self.revision.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for StreamSound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StreamSound")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
