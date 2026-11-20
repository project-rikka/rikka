use std::collections::VecDeque;
use std::sync::Arc;

use rikka_sync::{Condvar, Mutex, MutexGuard};
use rikka_worker::Stop;

use crate::worker::{Control, Event};

const DEPTH: usize = 3;

pub(crate) type Wake = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct Mailbox {
    post: Mutex<Post>,
    changed: Condvar,
    wake: Wake,
}

#[derive(Default)]
struct Post {
    events: VecDeque<Event>,
    controls: VecDeque<Control>,
    hungry: bool,
    closed: bool,
}

pub(crate) enum Delivery {
    Delivered,
    Interrupted(Event, Control),
}

impl Mailbox {
    pub(crate) fn new(wake: Wake) -> Mailbox {
        Mailbox {
            post: Mutex::new(Post::default()),
            changed: Condvar::new(),
            wake,
        }
    }

    pub(crate) fn control(&self) -> Option<Control> {
        let mut post = self.post();
        match post.closed {
            true => Some(Control::Stop),
            false => post.controls.pop_front(),
        }
    }

    pub(crate) fn wait_control(&self) -> Control {
        let mut post = self.post();
        loop {
            if post.closed {
                return Control::Stop;
            }
            if let Some(control) = post.controls.pop_front() {
                return control;
            }
            post = self.changed.wait(post);
        }
    }

    pub(crate) fn deliver(&self, event: Event) -> Delivery {
        let mut post = self.post();
        loop {
            if post.closed {
                return Delivery::Interrupted(event, Control::Stop);
            }
            if let Some(control) = post.controls.pop_front() {
                return Delivery::Interrupted(event, control);
            }
            if post.events.len() < DEPTH {
                post.events.push_back(event);
                let hungry = std::mem::take(&mut post.hungry);
                drop(post);
                if hungry {
                    (self.wake)();
                }
                return Delivery::Delivered;
            }
            post = self.changed.wait(post);
        }
    }

    pub(crate) fn send(&self, control: Control) {
        self.post().controls.push_back(control);
        self.changed.notify_all();
    }

    pub(crate) fn take(&self) -> Option<Event> {
        let mut post = self.post();
        let taken = post.events.pop_front();
        post.hungry = taken.is_none();
        drop(post);
        if taken.is_some() {
            self.changed.notify_all();
        }
        taken
    }

    pub(crate) fn close(&self) {
        let mut post = self.post();
        post.closed = true;
        post.events.clear();
        drop(post);
        self.changed.notify_all();
    }

    fn post(&self) -> MutexGuard<'_, Post> {
        self.post.lock()
    }
}

impl Stop for Mailbox {
    fn stop(&self) {
        self.close();
    }
}
