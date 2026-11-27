use std::sync::Arc;

use rikka_sync::Mutex;

pub(super) type Listener = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
pub(super) struct Progress {
    listeners: Mutex<Vec<Listener>>,
}

impl Progress {
    pub(super) fn listen(&self, done: impl FnOnce() -> bool, listener: &Listener) {
        let mut listeners = self.listeners.lock();
        if done() {
            drop(listeners);
            listener();
            return;
        }
        if !listeners.iter().any(|known| Arc::ptr_eq(known, listener)) {
            listeners.push(Arc::clone(listener));
        }
    }

    pub(super) fn advance(&self) {
        let listeners = std::mem::take(&mut *self.listeners.lock());
        for listener in listeners {
            listener();
        }
    }
}

impl std::fmt::Debug for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Progress")
            .field("listeners", &self.listeners.lock().len())
            .finish()
    }
}
