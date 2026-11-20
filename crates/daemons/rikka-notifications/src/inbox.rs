use rikka_sync::Mutex;
use std::collections::HashSet;

use crate::notification::Notification;
use crate::posted::Posted;

pub(crate) struct Inbox {
    state: Mutex<State>,
    wake: Box<dyn Fn() + Send + Sync>,
}

#[derive(Debug, Default)]
struct State {
    posted: Vec<Posted>,
    open: HashSet<u32>,
    last: u32,
}

impl Inbox {
    #[must_use]
    pub(crate) fn new(wake: Box<dyn Fn() + Send + Sync>) -> Inbox {
        Inbox {
            state: Mutex::new(State::default()),
            wake,
        }
    }

    pub(crate) fn post(&self, replaces: u32, mut notification: Notification) -> u32 {
        let id = {
            let mut state = self.lock();
            let id = state.allocate_id(replaces);
            state.open.insert(id);
            notification.id = id;
            state.posted.push(Posted::Shown(Box::new(notification)));
            id
        };
        (self.wake)();
        id
    }

    pub(crate) fn withdraw(&self, id: u32) {
        self.lock().posted.push(Posted::Withdrawn(id));
        (self.wake)();
    }

    pub(crate) fn close(&self, id: u32) -> bool {
        self.lock().open.remove(&id)
    }

    pub(crate) fn is_open(&self, id: u32) -> bool {
        self.lock().open.contains(&id)
    }

    pub(crate) fn drain(&self) -> Vec<Posted> {
        std::mem::take(&mut self.lock().posted)
    }

    fn lock(&self) -> rikka_sync::MutexGuard<'_, State> {
        self.state.lock()
    }
}

impl State {
    fn allocate_id(&mut self, replaces: u32) -> u32 {
        if replaces == 0 {
            self.last = self.last.checked_add(1).unwrap_or(1);
            return self.last;
        }
        self.last = self.last.max(replaces);
        replaces
    }
}

impl std::fmt::Debug for Inbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inbox")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}
