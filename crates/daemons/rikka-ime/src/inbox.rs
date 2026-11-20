use rikka_sync::{Mutex, MutexGuard};
use std::sync::Arc;

use crate::composing::Composing;
use crate::state::State;
use crate::status_area::StatusArea;

pub(crate) struct Inbox {
    state: Mutex<State>,
    shown: Mutex<Snapshot>,
    wake: Box<dyn Fn() + Send + Sync>,
}

#[derive(Default)]
struct Snapshot {
    composing: Arc<Composing>,
    status_area: Arc<StatusArea>,
}

impl Inbox {
    pub(crate) fn new(wake: Box<dyn Fn() + Send + Sync>) -> Inbox {
        Inbox {
            state: Mutex::new(State::default()),
            shown: Mutex::new(Snapshot::default()),
            wake,
        }
    }

    pub(crate) fn update(&self, fold: impl FnOnce(&mut State)) {
        let moved = {
            let mut state = self.state.lock();
            fold(&mut state);
            let mut shown = self.snapshot();
            let composing = state.composing();
            let composed = *shown.composing != composing;
            if composed {
                shown.composing = Arc::new(composing);
            }
            let restated = !state.shows_status_area(&shown.status_area);
            if restated {
                shown.status_area = Arc::new(state.status_area());
            }
            composed || restated
        };
        if moved {
            (self.wake)();
        }
    }

    #[must_use]
    pub(crate) fn composing(&self) -> Arc<Composing> {
        Arc::clone(&self.snapshot().composing)
    }

    #[must_use]
    pub(crate) fn status_area(&self) -> Arc<StatusArea> {
        Arc::clone(&self.snapshot().status_area)
    }

    #[must_use]
    pub(crate) fn is_composing(&self) -> bool {
        !self.snapshot().composing.is_empty()
    }

    fn snapshot(&self) -> MutexGuard<'_, Snapshot> {
        self.shown.lock()
    }
}

impl std::fmt::Debug for Inbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inbox").finish_non_exhaustive()
    }
}
