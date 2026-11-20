use rikka_sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::handle::PlaybackHandle;

type Listener = dyn Fn(PlaybackEvent) + Send + Sync;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlaybackEvent {
    Progressed,
    Finished,
}

#[must_use]
pub struct PlaybackWatch {
    _listener: Arc<Listener>,
}

impl std::fmt::Debug for PlaybackWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlaybackWatch").finish_non_exhaustive()
    }
}

#[derive(Default)]
pub(crate) struct Listeners {
    listeners: Mutex<Vec<Weak<Listener>>>,
    finished: AtomicBool,
}

impl Listeners {
    pub(crate) fn add(&self, listener: Arc<Listener>) -> PlaybackWatch {
        let mut listeners = self.listeners.lock();
        listeners.retain(|listener| listener.strong_count() > 0);
        listeners.push(Arc::downgrade(&listener));
        PlaybackWatch {
            _listener: listener,
        }
    }

    pub(crate) fn is_heard(&self) -> bool {
        self.listeners
            .lock()
            .iter()
            .any(|listener| listener.strong_count() > 0)
    }

    pub(crate) fn has_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    pub(crate) fn finish(&self) {
        if !self.finished.swap(true, Ordering::AcqRel) {
            self.tell(PlaybackEvent::Finished);
        }
    }

    pub(crate) fn tell(&self, event: PlaybackEvent) {
        let listeners: Vec<Arc<Listener>> = {
            let mut listeners = self.listeners.lock();
            listeners.retain(|listener| listener.strong_count() > 0);
            listeners.iter().filter_map(Weak::upgrade).collect()
        };
        for listener in listeners {
            listener(event);
        }
    }
}

impl std::fmt::Debug for Listeners {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listeners")
            .field("finished", &self.has_finished())
            .finish_non_exhaustive()
    }
}

struct Watching {
    handle: PlaybackHandle,
    told: Duration,
}

#[derive(Default)]
pub(crate) struct Watched(Mutex<Vec<Watching>>);

impl Watched {
    pub(crate) fn add(&self, handle: PlaybackHandle) {
        let mut watching = self.0.lock();
        if watching
            .iter()
            .any(|known| Arc::ptr_eq(&known.handle.shared, &handle.shared))
        {
            return;
        }
        let told = handle.position();
        watching.push(Watching { handle, told });
    }

    pub(crate) fn report(&self) {
        let mut moved = Vec::new();
        self.0.lock().retain_mut(|watching| {
            let shared = &watching.handle.shared;
            if shared.listeners.has_finished() || !shared.listeners.is_heard() {
                return false;
            }
            let position = watching.handle.position();
            if !watching.handle.is_paused() && position != watching.told {
                watching.told = position;
                moved.push(Arc::clone(shared));
            }
            true
        });
        for shared in moved {
            shared.listeners.tell(PlaybackEvent::Progressed);
        }
    }
}

impl std::fmt::Debug for Watched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watched")
            .field("watching", &self.0.lock().len())
            .finish()
    }
}
