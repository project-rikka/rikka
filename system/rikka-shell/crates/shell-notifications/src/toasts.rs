use std::time::{Duration, Instant};

use rikka_notifications::{CloseReason, DEFAULT_ACTION};
use rikka_seam::{Payload, SeamError};
use rikka_session::following;
use rikka_ui::{Handler, State, after, skip_view, spawn, stream};
use rikka_widgets::Dismissible;
use shell_kit::launch::{self, Spawn};

use crate::service::{CAPABILITY, CHANGED};
use crate::shared::Shared;
use crate::stack::Stack;

#[derive(Debug, State)]
pub(super) struct Toasts {
    shared: Shared,
    hovered: bool,
}

impl Toasts {
    #[must_use]
    pub(crate) fn new(shared: Shared) -> Toasts {
        Toasts {
            shared,
            hovered: false,
        }
    }

    pub(crate) fn stack(&self) -> &Stack {
        &self.shared.stack
    }

    pub(crate) fn start(&mut self) {
        self.shared.stack.rearm(Instant::now());
        let announced = following::changes(&self.shared.seam, CAPABILITY, CHANGED);
        stream(announced, |toasts: &mut Toasts, ()| toasts.settle());
        self.settle();
    }

    fn settle(&mut self) {
        let now = Instant::now();
        let hovered = self.hovered;
        let waits: Vec<(Duration, Handler)> = self.shared.stack.each(|toast| {
            let id = toast.id();
            if toast.leaving {
                return toast.begin_exit().map(|generation| {
                    let done = Handler::new(move |toasts: &mut Toasts| {
                        toasts.remove_exited(id, generation)
                    });
                    (Dismissible::DEPARTURE, done)
                });
            }
            if hovered {
                return None;
            }
            toast.start(now).map(|(left, countdown)| {
                let done = Handler::new(move |toasts: &mut Toasts| toasts.expired(id, countdown));
                (left, done)
            })
        });
        for (delay, done) in waits {
            after(delay, done);
        }
    }

    pub(crate) fn hover(&mut self, inside: bool) {
        if self.hovered == inside {
            return skip_view();
        }
        self.hovered = inside;
        if !inside {
            return self.settle();
        }
        self.shared.stack.pause(Instant::now());
        skip_view();
    }

    fn expired(&mut self, id: u32, countdown: u64) {
        let over = self
            .shared
            .stack
            .one(id, |toast| toast.is_over(countdown))
            .unwrap_or_default();
        if !over {
            return skip_view();
        }
        self.shelve(id, CloseReason::Expired);
    }

    fn remove_exited(&mut self, id: u32, generation: u64) {
        self.shared.stack.remove(id, generation);
        self.settle();
    }

    pub(crate) fn put_away(&mut self, id: u32) {
        self.shelve(id, CloseReason::Dismissed);
    }

    fn shelve(&mut self, id: u32, unkept: CloseReason) {
        let stored_notification = self
            .shared
            .stack
            .one(id, |toast| {
                (!toast.notification.transient).then(|| toast.stored_notification())
            })
            .flatten();
        let Some(stored_notification) = stored_notification else {
            return self.close(id, unkept);
        };
        self.shared.history.keep(stored_notification);
        self.shared.stack.dismiss(id);
        self.settle();
    }

    pub(crate) fn dismiss(&mut self, id: u32) {
        self.close(id, CloseReason::Dismissed);
    }

    fn close(&mut self, id: u32, reason: CloseReason) {
        self.shared.history.release(id);
        self.shared.action_router.close(id, reason);
        self.shared.stack.dismiss(id);
        self.settle();
    }

    pub(crate) fn activate(&mut self, id: u32) {
        let has_default_action = self
            .shared
            .stack
            .one(id, |toast| toast.notification.default_action().is_some())
            .unwrap_or_default();
        match has_default_action {
            true => self.act(id, DEFAULT_ACTION),
            false => self.close(id, CloseReason::Dismissed),
        }
    }

    pub(crate) fn act(&mut self, id: u32, key: &str) {
        self.shared.action_router.activate(id, key);
        let stays = self
            .shared
            .stack
            .one(id, |toast| toast.notification.resident)
            .unwrap_or_default();
        match stays {
            true => self.settle(),
            false => self.close(id, CloseReason::Dismissed),
        }
    }

    pub(crate) fn follow(&mut self, id: u32, at: usize) {
        let href = self
            .shared
            .stack
            .one(id, |toast| {
                toast
                    .notification
                    .body
                    .links
                    .get(at)
                    .map(|link| link.href.clone())
            })
            .flatten();
        let Some(href) = href else {
            return skip_view();
        };
        let has_default_action = Spawn::new(rikka_open::opener(&href)).request();
        spawn(
            self.shared.seam.call(has_default_action),
            |reply: Result<Payload, SeamError>| {
                if let Err(error) = reply {
                    tracing::warn!(
                        "seam call {}/{} failed: {error}",
                        launch::CAPABILITY,
                        launch::SPAWN
                    );
                }
                skip_view();
            },
        );
    }
}
