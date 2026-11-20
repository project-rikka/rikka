use std::fmt;
use std::rc::Weak;

use crate::reactor::{Reactor, Shared};

pub struct WeakReactor<S: 'static> {
    shared: Weak<Shared<S>>,
}

impl<S: 'static> WeakReactor<S> {
    pub(crate) fn new(shared: Weak<Shared<S>>) -> Self {
        Self { shared }
    }

    pub fn upgrade(&self) -> Option<Reactor<S>> {
        self.shared.upgrade().map(Reactor::from_shared)
    }
}

impl<S: 'static> Clone for WeakReactor<S> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<S: 'static> fmt::Debug for WeakReactor<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WeakReactor")
            .field("alive", &(self.shared.strong_count() > 0))
            .finish()
    }
}
