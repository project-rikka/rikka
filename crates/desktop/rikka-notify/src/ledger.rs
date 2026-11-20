use std::collections::HashMap;

use crate::event::{Event, Id};

#[derive(Clone, Debug)]
pub struct Ledger<T> {
    shown: HashMap<Id, T>,
    tokens: HashMap<Id, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invoked<T> {
    pub shown: T,
    pub key: String,
    pub token: Option<String>,
}

impl<T> Default for Ledger<T> {
    fn default() -> Ledger<T> {
        Ledger {
            shown: HashMap::new(),
            tokens: HashMap::new(),
        }
    }
}

impl<T> Ledger<T> {
    #[must_use]
    pub fn new() -> Ledger<T> {
        Ledger::default()
    }

    pub fn record(&mut self, id: Id, shown: T) {
        self.shown.insert(id, shown);
    }

    pub fn settle(&mut self, event: Event) -> Option<Invoked<T>> {
        match event {
            Event::ActivationToken { id, token } => {
                self.tokens.insert(id, token);
                None
            }
            Event::ActionInvoked { id, key } => {
                let token = self.tokens.remove(&id);
                let shown = self.shown.remove(&id)?;
                Some(Invoked { shown, key, token })
            }
            Event::Closed { id, .. } => {
                self.remove(id);
                None
            }
        }
    }

    #[must_use]
    pub fn find(&self, mut wanted: impl FnMut(&T) -> bool) -> Option<Id> {
        self.shown
            .iter()
            .find(|(_, shown)| wanted(shown))
            .map(|(id, _)| *id)
    }

    pub fn remove(&mut self, id: Id) -> Option<T> {
        self.tokens.remove(&id);
        self.shown.remove(&id)
    }
}
