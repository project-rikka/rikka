use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::sync::Arc;

use super::cache::{Cache, Window};
use crate::read::VideoInfo;

pub(super) type Wake = Arc<dyn Fn() + Send + Sync>;

pub(super) struct Shared<T> {
    state: Mutex<State<T>>,
    changed: Condvar,
}

pub(super) struct State<T> {
    pub(super) info: Option<VideoInfo>,
    pub(super) failure: Option<String>,
    pub(super) window: Option<Window>,
    pub(super) bounds: Option<(u32, u32)>,
    pub(super) fit: u64,
    pub(super) cache: Cache<T>,
    pub(super) ready: Option<Wake>,
    pub(super) closed: bool,
}

impl<T> State<T> {
    pub(super) fn missing(&self) -> Option<std::time::Duration> {
        self.window
            .and_then(|window| self.cache.missing(window, self.fit))
    }
}

impl<T> Shared<T> {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(State {
                info: None,
                failure: None,
                window: None,
                bounds: None,
                fit: 0,
                cache: Cache::new(),
                ready: None,
                closed: false,
            }),
            changed: Condvar::new(),
        }
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock()
    }

    pub(super) fn wait<'a>(&self, state: MutexGuard<'a, State<T>>) -> MutexGuard<'a, State<T>> {
        self.changed.wait(state)
    }

    pub(super) fn change(&self, change: impl FnOnce(&mut State<T>) -> bool) {
        let changed = change(&mut self.lock());
        if changed {
            self.changed.notify_all();
        }
    }
}
