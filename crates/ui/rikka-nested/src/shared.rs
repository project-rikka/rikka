use rikka_sync::Mutex;
use std::sync::Arc;

use rikka_ui::Pulse;

use crate::frame::Frame;
use crate::look::Look;

pub(crate) type Watcher = Box<dyn Fn() + Send + Sync>;

pub(crate) struct Shared {
    frame: Mutex<Arc<Frame>>,
    look: Mutex<Arc<Look>>,
    watcher: Mutex<Option<Watcher>>,
    pulse: Pulse,
}

impl Shared {
    pub(crate) fn new(pulse: Pulse) -> Shared {
        Shared {
            frame: Mutex::default(),
            look: Mutex::default(),
            watcher: Mutex::default(),
            pulse,
        }
    }

    pub(crate) fn frame(&self) -> Arc<Frame> {
        Arc::clone(&self.frame.lock())
    }

    pub(crate) fn look(&self) -> Arc<Look> {
        Arc::clone(&self.look.lock())
    }

    pub(crate) fn pulse(&self) -> &Pulse {
        &self.pulse
    }

    pub(crate) fn show(&self, frame: Frame) {
        *self.frame.lock() = Arc::new(frame);
        self.wake();
    }

    pub(crate) fn change(&self, change: impl FnOnce(&mut Look)) {
        change(Arc::make_mut(&mut self.look.lock()));
        self.wake();
    }

    pub(crate) fn watch(&self, watcher: Watcher) {
        *self.watcher.lock() = Some(watcher);
    }

    pub(crate) fn wake(&self) {
        if let Some(watcher) = self.watcher.lock().as_ref() {
            watcher();
        }
    }
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Shared")
            .field("generation", &self.frame.lock().generation)
            .finish_non_exhaustive()
    }
}
