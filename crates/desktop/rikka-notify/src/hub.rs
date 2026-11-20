use rikka_sync::Mutex;
use std::collections::HashSet;

use rikka_worker::{Broadcast, Subscription};

use crate::event::{Event, Id};

pub type Events = Subscription<Event>;

#[derive(Debug, Default)]
pub(crate) struct Hub {
    ours: Mutex<HashSet<Id>>,
    listeners: Broadcast<Event>,
}

impl Hub {
    pub(crate) fn claim(&self, id: Id) {
        self.ours.lock().insert(id);
    }

    pub(crate) fn listen(&self) -> Events {
        self.listeners.subscribe()
    }

    pub(crate) fn dispatch(&self, event: Event) {
        let id = event.id();
        let ours = match event {
            Event::Closed { .. } => self.ours.lock().remove(&id),
            _ => self.ours.lock().contains(&id),
        };
        if ours {
            self.listeners.send(&event);
        }
    }
}
