use std::time::{Duration, Instant};

use rikka_notifications::Notification;

use crate::look::Look;
use crate::stored::StoredNotification;

#[derive(Debug)]
pub(super) struct Toast {
    pub(crate) notification: Notification,
    pub(crate) look: Look,
    left: Option<Duration>,
    since: Option<Instant>,
    generation: u64,
    pub(crate) leaving: bool,
    exiting: bool,
}

impl Toast {
    #[must_use]
    pub(crate) fn new(notification: Notification, linger: Option<Duration>, look: Look) -> Toast {
        Toast {
            notification,
            look,
            left: linger,
            since: None,
            generation: 0,
            leaving: false,
            exiting: false,
        }
    }

    #[must_use]
    pub(crate) fn id(&self) -> u32 {
        self.notification.id
    }

    #[must_use]
    pub(crate) fn stored_notification(&self) -> StoredNotification {
        StoredNotification::new(self.notification.clone(), self.look.clone())
    }

    pub(crate) fn replace(
        &mut self,
        notification: Notification,
        linger: Option<Duration>,
        look: Look,
    ) {
        self.notification = notification;
        self.look = look;
        self.left = linger;
        self.since = None;
        self.generation += 1;
        self.leaving = false;
        self.exiting = false;
    }

    pub(crate) fn refresh(&mut self, notification: Notification, look: Look) {
        self.notification = notification;
        self.look = look;
    }

    #[must_use]
    pub(crate) fn start(&mut self, now: Instant) -> Option<(Duration, u64)> {
        if self.leaving || self.since.is_some() {
            return None;
        }
        let left = self.left?;
        self.since = Some(now);
        self.generation += 1;
        Some((left, self.generation))
    }

    pub(crate) fn pause(&mut self, now: Instant) {
        let Some(since) = self.since.take() else {
            return;
        };
        self.left = self
            .left
            .map(|left| left.saturating_sub(now.saturating_duration_since(since)));
        self.generation += 1;
    }

    #[must_use]
    pub(super) fn is_over(&self, generation: u64) -> bool {
        !self.leaving && self.since.is_some() && self.generation == generation
    }

    pub(crate) fn leave(&mut self) {
        if self.leaving {
            return;
        }
        self.leaving = true;
        self.since = None;
        self.generation += 1;
        self.exiting = false;
    }

    #[must_use]
    pub(crate) fn begin_exit(&mut self) -> Option<u64> {
        (!std::mem::replace(&mut self.exiting, true)).then_some(self.generation)
    }

    #[must_use]
    pub(super) fn has_exited(&self, generation: u64) -> bool {
        self.leaving && self.generation == generation
    }

    pub(super) fn rearm(&mut self, now: Instant) {
        self.pause(now);
        self.exiting = false;
    }
}
