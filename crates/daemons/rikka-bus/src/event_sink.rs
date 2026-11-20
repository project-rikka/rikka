use std::sync::Arc;

pub struct EventSink<E>(Arc<dyn Fn(E) + Send + Sync>);

impl<E> EventSink<E> {
    #[must_use]
    pub fn new(callback: impl Fn(E) + Send + Sync + 'static) -> EventSink<E> {
        EventSink(Arc::new(callback))
    }

    pub fn emit(&self, event: E) {
        (self.0)(event);
    }
}

impl<E> Clone for EventSink<E> {
    fn clone(&self) -> EventSink<E> {
        EventSink(Arc::clone(&self.0))
    }
}

impl<E> std::fmt::Debug for EventSink<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("EventSink").finish_non_exhaustive()
    }
}
