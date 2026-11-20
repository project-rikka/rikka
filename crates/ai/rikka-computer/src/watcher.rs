use std::sync::Arc;

use crate::frame::Frame;
use crate::status::Status;

#[derive(Debug)]
pub enum Event {
    Status(Status),
    Frame(Frame),
    Disconnected,
}

#[derive(Clone)]
pub(crate) struct Watcher {
    callback: Arc<dyn Fn(Event) + Send + Sync>,
}

impl Watcher {
    pub(crate) fn new(callback: Arc<dyn Fn(Event) + Send + Sync>) -> Watcher {
        Watcher { callback }
    }

    pub(crate) fn notify(&self, event: Event) {
        (self.callback)(event);
    }
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Watcher").finish_non_exhaustive()
    }
}
