use rikka_sync::{Mutex, MutexGuard};
use std::sync::Arc;
use std::time::Duration;

use replay_ring::{PictureId, Ring};
use rikka_session::replay::Kind;
use rikka_video::StreamInfo;

pub(crate) const SOUNDS: usize = 2;
pub(crate) const DESKTOP: usize = 0;
pub(crate) const MICROPHONE: usize = 1;

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) subject: String,
    pub(crate) kind: Kind,
    pub(crate) pictures: PictureId,
    pub(crate) stream: Arc<StreamInfo>,
}

#[derive(Debug)]
pub(crate) struct Store {
    pub(crate) ring: Ring,
    entries: Vec<Entry>,
    sounds: [Option<Arc<StreamInfo>>; SOUNDS],
}

#[derive(Clone, Debug)]
pub(crate) struct SharedStore(Arc<Mutex<Store>>);

impl SharedStore {
    #[must_use]
    pub(crate) fn new(keep: Duration, budget: usize) -> SharedStore {
        SharedStore(Arc::new(Mutex::new(Store {
            ring: Ring::new(keep, budget, SOUNDS),
            entries: Vec::new(),
            sounds: [None, None],
        })))
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Store> {
        self.0.lock()
    }
}

impl Store {
    pub(crate) fn push_entry(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    pub(crate) fn set_kind(&mut self, pictures: PictureId, kind: Kind) {
        for entry in &mut self.entries {
            if entry.pictures == pictures {
                entry.kind = kind.clone();
            }
        }
    }

    pub(crate) fn end(&mut self, pictures: PictureId) {
        self.ring.end_pictures(pictures);
    }

    pub(crate) fn trim(&mut self, now_us: i64) {
        self.ring.trim(now_us);
        let live: Vec<PictureId> = self.ring.pictures().collect();
        self.entries.retain(|entry| live.contains(&entry.pictures));
    }

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    #[must_use]
    pub(crate) fn latest(&self, subject: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.subject == subject && self.ring.span(entry.pictures).is_some())
    }

    pub(crate) fn set_sound_stream(&mut self, track: usize, stream: Option<Arc<StreamInfo>>) {
        self.ring.clear_sound(track);
        if let Some(slot) = self.sounds.get_mut(track) {
            *slot = stream;
        }
    }

    #[must_use]
    pub(crate) fn sound(&self, track: usize) -> Option<Arc<StreamInfo>> {
        self.sounds.get(track).cloned().flatten()
    }

    pub(crate) fn clear(&mut self) {
        let (keep, budget) = (self.ring.window(), self.ring.budget());
        self.ring = Ring::new(keep, budget, SOUNDS);
        self.entries.clear();
    }
}
