use rikka_sync::Mutex;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::Poll;

use futures_util::{Stream, StreamExt, stream};

use browser_model::WindowId;

type Feed<T> = Pin<Box<dyn Stream<Item = T> + Send>>;

pub(crate) struct Relay<T>(Arc<RelayState<T>>);

struct RelayState<T> {
    feed: Mutex<Feed<T>>,
    listener: AtomicU64,
}

impl<T: Send + 'static> Relay<T> {
    pub(crate) fn new(feed: impl Stream<Item = T> + Send + 'static) -> Relay<T> {
        Relay(Arc::new(RelayState {
            feed: Mutex::new(Box::pin(feed)),
            listener: AtomicU64::new(0),
        }))
    }

    pub(crate) fn stream(&self) -> impl Stream<Item = T> + Send + 'static + use<T> {
        let state = Arc::clone(&self.0);
        let listener = state.listener.fetch_add(1, Ordering::AcqRel) + 1;
        stream::poll_fn(move |context| {
            let mut feed = state.feed.lock();
            if state.listener.load(Ordering::Acquire) != listener {
                return Poll::Ready(None);
            }
            feed.poll_next_unpin(context)
        })
    }
}

impl<T> std::fmt::Debug for Relay<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Relay").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(crate) struct WindowFeed<T> {
    relay: Relay<T>,
    listener: Option<WindowId>,
}

impl<T: Send + 'static> WindowFeed<T> {
    pub(crate) fn new(feed: impl Stream<Item = T> + Send + 'static) -> WindowFeed<T> {
        WindowFeed {
            relay: Relay::new(feed),
            listener: None,
        }
    }

    pub(crate) fn subscribe(
        &mut self,
        window: WindowId,
        alive: &[WindowId],
    ) -> Option<impl Stream<Item = T> + Send + 'static + use<T>> {
        if self
            .listener
            .is_some_and(|listener| alive.contains(&listener))
        {
            return None;
        }
        self.listener = Some(window);
        Some(self.relay.stream())
    }
}
