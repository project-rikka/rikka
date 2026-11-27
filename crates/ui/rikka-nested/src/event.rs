use rikka_sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::claim::ToplevelKey;
use crate::look::{Constraint, TextInput};
use crate::view::ViewId;

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    ClientConnected {
        pid: i32,
    },
    ClientRefused {
        uid: u32,
    },
    ClientGone {
        pid: i32,
        reason: String,
    },
    Unclaimed {
        toplevel: ToplevelKey,
        title: Option<String>,
    },
    UnclaimedClosed {
        toplevel: ToplevelKey,
    },
    Attached {
        view: ViewId,
    },
    Detached {
        view: ViewId,
    },
    Title {
        view: ViewId,
        title: String,
    },
    ActivationRequested {
        view: ViewId,
    },
    FullscreenRequested {
        view: ViewId,
        fullscreen: bool,
    },
    TextInput {
        view: ViewId,
        state: Option<TextInput>,
    },
    PointerConstraint {
        view: ViewId,
        constraint: Option<Constraint>,
    },
    IdleInhibited(bool),
    DragStarted {
        view: ViewId,
        mime_types: Vec<String>,
    },
    DragEnded {
        view: ViewId,
        dropped: bool,
    },
}

#[derive(Debug)]
pub struct Events(Receiver<Event>);

impl Iterator for Events {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        self.0.recv().ok()
    }
}

#[derive(Debug, Default)]
enum Channel {
    #[default]
    Untaken,
    Open(Sender<Event>),
    Ended,
}

#[derive(Debug, Default)]
pub(crate) struct EventSender(Mutex<Channel>);

impl EventSender {
    pub(crate) fn take(&self) -> Option<Events> {
        let mut channel = self.channel();
        if !matches!(*channel, Channel::Untaken) {
            return None;
        }
        let (sender, receiver) = mpsc::channel();
        *channel = Channel::Open(sender);
        Some(Events(receiver))
    }

    pub(crate) fn send(&self, event: Event) {
        let mut channel = self.channel();
        if let Channel::Open(open) = &*channel
            && open.send(event).is_err()
        {
            *channel = Channel::Ended;
        }
    }

    pub(crate) fn close(&self) {
        *self.channel() = Channel::Ended;
    }

    fn channel(&self) -> rikka_sync::MutexGuard<'_, Channel> {
        self.0.lock()
    }
}
