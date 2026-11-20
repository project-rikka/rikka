use rikka_sync::{Condvar, Mutex};
use std::sync::Arc;
use std::time::Duration;

use rikka_ui::Ui;

pub(crate) struct Wakes(Arc<(Mutex<bool>, Condvar)>);

impl Wakes {
    pub(crate) fn wait(&self, timeout: Duration) -> bool {
        let (woken, ready) = &*self.0;
        let guard = woken.lock();
        let (mut guard, _) = ready.wait_timeout_while(guard, timeout, |woken| !*woken);
        std::mem::take(&mut *guard)
    }
}

pub(crate) fn wakes(ui: &mut Ui) -> Wakes {
    let shared = Arc::new((Mutex::new(false), Condvar::new()));
    let signal = Arc::clone(&shared);
    ui.wakes_with(Arc::new(move || {
        let (woken, ready) = &*signal;
        *woken.lock() = true;
        ready.notify_all();
    }));
    Wakes(shared)
}
