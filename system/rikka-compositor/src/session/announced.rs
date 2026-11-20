use std::fmt;

use rikka_seam::{Call, Publisher};
use serde::Serialize;

pub(crate) struct Announced<S> {
    event: &'static str,
    state: S,
    publisher: Option<Publisher>,
}

impl<S: Default + PartialEq + Serialize> Announced<S> {
    pub(crate) fn new(event: &'static str) -> Self {
        Self {
            event,
            state: S::default(),
            publisher: None,
        }
    }

    pub(crate) fn publish(&mut self, state: S) {
        if self.state == state {
            return;
        }
        self.state = state;
        if let Some(publisher) = &self.publisher
            && let Err(error) = publisher.emit(self.event, &self.state)
        {
            tracing::warn!("cannot publish `{}`: {error}", self.event);
        }
    }

    pub(crate) fn current(&self) -> &S {
        &self.state
    }

    pub(crate) fn attach(&mut self, publisher: Publisher) {
        self.publisher = Some(publisher);
    }

    pub(crate) fn answer(&self, call: Call) {
        call.encode(&self.state);
    }
}

impl<S: fmt::Debug> fmt::Debug for Announced<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Announced")
            .field("event", &self.event)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}
