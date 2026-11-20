use std::cell::RefCell;
use std::rc::Rc;

use futures_util::Stream;
use rikka_notifications::{CloseReason, DEFAULT_ACTION};
use rikka_worker::Observable;

use crate::action_router::ActionRouter;
use crate::stored::StoredNotification;

const MAX_KEPT: usize = 100;

#[derive(Clone, Debug, Default)]
pub struct History {
    state: Rc<RefCell<State>>,
    moved: Observable<()>,
}

#[derive(Debug, Default)]
pub(crate) struct State {
    entries: Vec<Rc<StoredNotification>>,
    unread: bool,
    action_router: Option<ActionRouter>,
}

impl History {
    pub(super) fn answers_with(&self, action_router: ActionRouter) {
        self.state.borrow_mut().action_router = Some(action_router);
    }

    pub(crate) fn keep(&self, kept: StoredNotification) {
        {
            let mut state = self.state.borrow_mut();
            state.entries.retain(|entry| entry.id() != kept.id());
            state.entries.insert(0, Rc::new(kept));
            state.unread = true;
            while state.entries.len() > MAX_KEPT {
                let dropped = state
                    .entries
                    .pop()
                    .expect("the list is longer than the cap");
                state.close(dropped.id(), CloseReason::Expired);
            }
            state.settle();
        }
        self.announce();
    }

    #[must_use]
    pub(crate) fn contains(&self, id: u32) -> bool {
        self.state
            .borrow()
            .entries
            .iter()
            .any(|entry| entry.id() == id)
    }

    pub(crate) fn refresh(&self, fresh: StoredNotification) {
        {
            let mut state = self.state.borrow_mut();
            let Some(entry) = state
                .entries
                .iter_mut()
                .find(|entry| entry.id() == fresh.id())
            else {
                return;
            };
            *entry = Rc::new(fresh);
        }
        self.announce();
    }

    pub fn forget(&self, id: u32) {
        self.take(id, Some(CloseReason::Dismissed));
    }

    pub(crate) fn release(&self, id: u32) {
        self.take(id, None);
    }

    pub fn clear(&self) {
        {
            let mut state = self.state.borrow_mut();
            if state.entries.is_empty() {
                return;
            }
            for entries in std::mem::take(&mut state.entries) {
                state.close(entries.id(), CloseReason::Dismissed);
            }
            state.settle();
        }
        self.announce();
    }

    #[must_use]
    pub fn activate(&self, id: u32) -> bool {
        let has_default_action = self
            .one(id, |entries| {
                entries.notification.default_action().is_some()
            })
            .unwrap_or_default();
        if !has_default_action {
            self.forget(id);
            return false;
        }
        self.invoke(id, DEFAULT_ACTION)
    }

    pub fn invoke(&self, id: u32, key: &str) -> bool {
        let stays = {
            let state = self.state.borrow();
            let Some(entries) = state.entry(id) else {
                return false;
            };
            let stays = entries.notification.resident;
            state.activate(id, key);
            stays
        };
        if !stays {
            self.forget(id);
        }
        !stays
    }

    #[must_use]
    pub fn link(&self, id: u32, at: usize) -> Option<String> {
        self.one(id, |entries| {
            entries
                .notification
                .body
                .links
                .get(at)
                .map(|link| link.href.clone())
        })
        .flatten()
    }

    #[must_use]
    pub(crate) fn read<R>(&self, read: impl FnOnce(&[Rc<StoredNotification>]) -> R) -> R {
        read(&self.state.borrow().entries)
    }

    #[must_use]
    pub(crate) fn one<R>(&self, id: u32, read: impl FnOnce(&StoredNotification) -> R) -> Option<R> {
        self.state.borrow().entry(id).map(read)
    }

    #[must_use]
    pub fn is_unread(&self) -> bool {
        self.state.borrow().unread
    }

    pub fn mark_read(&self) {
        if !self.state.borrow().unread {
            return;
        }
        self.state.borrow_mut().unread = false;
        self.announce();
    }

    pub fn changes(&self) -> impl Stream<Item = ()> + Send + 'static {
        self.moved.changes()
    }

    fn take(&self, id: u32, reason: Option<CloseReason>) {
        {
            let mut state = self.state.borrow_mut();
            let Some(at) = state.entries.iter().position(|entries| entries.id() == id) else {
                return;
            };
            state.entries.remove(at);
            if let Some(reason) = reason {
                state.close(id, reason);
            }
            state.settle();
        }
        self.announce();
    }

    fn announce(&self) {
        self.moved.set(());
    }
}

impl State {
    #[must_use]
    fn entry(&self, id: u32) -> Option<&StoredNotification> {
        self.entries
            .iter()
            .find(|entry| entry.id() == id)
            .map(Rc::as_ref)
    }

    fn close(&self, id: u32, reason: CloseReason) {
        if let Some(action_router) = &self.action_router {
            action_router.close(id, reason);
        }
    }

    fn activate(&self, id: u32, key: &str) {
        if let Some(action_router) = &self.action_router {
            action_router.activate(id, key);
        }
    }

    fn settle(&mut self) {
        self.unread &= !self.entries.is_empty();
    }
}
