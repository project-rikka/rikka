use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_notifications::Notification;
use shell_kit::Presence;

use crate::look::Look;
use crate::toast::Toast;

const MAX_WAITING: usize = 32;

#[derive(Clone, Debug)]
pub(super) struct Stack(Rc<RefCell<State>>);

#[derive(Debug)]
pub(crate) struct State {
    shown: Vec<Toast>,
    waiting: VecDeque<Toast>,
    visible: usize,
    presence: Presence,
}

impl Stack {
    #[must_use]
    pub(crate) fn new(visible: usize, presence: Presence) -> Stack {
        Stack(Rc::new(RefCell::new(State {
            shown: Vec::new(),
            waiting: VecDeque::new(),
            visible,
            presence,
        })))
    }

    #[must_use]
    pub(crate) fn post(
        &self,
        notification: Notification,
        linger: Option<Duration>,
        look: Look,
    ) -> Option<Toast> {
        let mut state = self.0.borrow_mut();
        let id = notification.id;
        if let Some(entry) = state.entry_mut(id) {
            match notification.progress {
                Some(_) => entry.refresh(notification, look),
                None => entry.replace(notification, linger, look),
            }
            return None;
        }
        let toast = Toast::new(notification, linger, look);
        if state.shown.len() < state.visible {
            state.shown.insert(0, toast);
            state.settle();
            return None;
        }
        state.waiting.push_back(toast);
        state.settle();
        (state.waiting.len() > MAX_WAITING)
            .then(|| state.waiting.pop_front())
            .flatten()
    }

    pub(crate) fn dismiss(&self, id: u32) {
        let mut state = self.0.borrow_mut();
        if let Some(shown) = state.shown.iter_mut().find(|toast| toast.id() == id) {
            shown.leave();
            return;
        }
        state.waiting.retain(|toast| toast.id() != id);
        state.settle();
    }

    pub(crate) fn remove(&self, id: u32, generation: u64) {
        let mut state = self.0.borrow_mut();
        state
            .shown
            .retain(|toast| toast.id() != id || !toast.has_exited(generation));
        state.fill();
        state.settle();
    }

    pub(super) fn show_at_most(&self, visible: usize) {
        let mut state = self.0.borrow_mut();
        state.visible = visible;
        state.fill();
    }

    pub(super) fn rearm(&self, now: Instant) {
        for toast in &mut self.0.borrow_mut().shown {
            toast.rearm(now);
        }
    }

    #[must_use]
    pub(crate) fn read<R>(&self, read: impl FnOnce(&[Toast], usize) -> R) -> R {
        let state = self.0.borrow();
        read(&state.shown, state.waiting.len())
    }

    #[must_use]
    pub(crate) fn each<R>(&self, act: impl FnMut(&mut Toast) -> Option<R>) -> Vec<R> {
        let mut state = self.0.borrow_mut();
        state.shown.iter_mut().filter_map(act).collect()
    }

    pub(crate) fn pause(&self, now: Instant) {
        for toast in &mut self.0.borrow_mut().shown {
            toast.pause(now);
        }
    }

    #[must_use]
    pub(crate) fn one<R>(&self, id: u32, act: impl FnOnce(&mut Toast) -> R) -> Option<R> {
        let mut state = self.0.borrow_mut();
        state.entry_mut(id).map(act)
    }
}

impl State {
    fn entry_mut(&mut self, id: u32) -> Option<&mut Toast> {
        self.shown
            .iter_mut()
            .chain(self.waiting.iter_mut())
            .find(|toast| toast.id() == id)
    }

    fn fill(&mut self) {
        while self.shown.len() < self.visible {
            let Some(waiting) = self.waiting.pop_front() else {
                return;
            };
            self.shown.insert(0, waiting);
        }
    }

    fn settle(&self) {
        self.presence
            .set(!(self.shown.is_empty() && self.waiting.is_empty()));
    }
}
