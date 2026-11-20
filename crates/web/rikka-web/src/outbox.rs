use std::fmt;
use std::sync::{Arc, OnceLock};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

pub(crate) struct Outbox<T>(Arc<OnceLock<UnboundedSender<T>>>);

impl<T> Clone for Outbox<T> {
    fn clone(&self) -> Outbox<T> {
        Outbox(Arc::clone(&self.0))
    }
}

impl<T> Default for Outbox<T> {
    fn default() -> Outbox<T> {
        Outbox(Arc::new(OnceLock::new()))
    }
}

impl<T> fmt::Debug for Outbox<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Outbox")
            .field("taken", &self.0.get().is_some())
            .finish()
    }
}

impl<T> Outbox<T> {
    pub(crate) fn send(&self, message: T) {
        if let Some(sender) = self.0.get()
            && sender.unbounded_send(message).is_err()
        {
            tracing::debug!("event receiver dropped");
        }
    }

    pub(crate) fn open(&self) -> Option<UnboundedReceiver<T>> {
        let (sender, receiver) = unbounded();
        self.0.set(sender).is_ok().then_some(receiver)
    }

    pub(crate) fn close(&self) {
        if let Some(sender) = self.0.get() {
            sender.close_channel();
        }
    }
}
