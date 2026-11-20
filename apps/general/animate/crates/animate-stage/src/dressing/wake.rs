use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use animate_shader::Listener;
use rikka_physics::ProgressCallback;

#[derive(Default)]
pub(crate) struct Wake {
    due: Arc<AtomicBool>,
    notify: Option<ProgressCallback>,
    waiting: Cell<bool>,
}

impl Wake {
    pub(crate) fn notify_with(&mut self, notify: ProgressCallback) {
        self.notify = Some(notify);
    }

    pub(crate) fn listener(&self) -> Listener {
        let due = Arc::clone(&self.due);
        let notify = self.notify.clone();
        Arc::new(move || {
            due.store(true, Ordering::Release);
            if let Some(notify) = &notify {
                notify();
            }
        })
    }

    pub(crate) fn rouse(&self) -> impl FnOnce() + Send + 'static {
        let listener = self.listener();
        move || listener()
    }

    pub(crate) fn wait(&self) {
        self.waiting.set(true);
    }

    pub(crate) fn begin_frame(&self) {
        self.waiting.set(false);
    }

    pub(crate) fn is_waiting(&self) -> bool {
        self.waiting.get()
    }

    pub(crate) fn take(&self) -> bool {
        self.due.swap(false, Ordering::AcqRel)
    }
}

impl std::fmt::Debug for Wake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wake")
            .field("due", &self.due.load(Ordering::Acquire))
            .field("waiting", &self.is_waiting())
            .finish_non_exhaustive()
    }
}
