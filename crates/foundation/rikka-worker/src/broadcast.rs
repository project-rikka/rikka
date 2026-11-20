use std::pin::Pin;
use std::task::{Context, Poll};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::Stream;
use rikka_sync::Mutex;

pub struct Broadcast<T> {
    senders: Mutex<Vec<UnboundedSender<T>>>,
}

impl<T> Default for Broadcast<T> {
    fn default() -> Broadcast<T> {
        Broadcast {
            senders: Mutex::new(Vec::new()),
        }
    }
}

impl<T: Clone> Broadcast<T> {
    #[must_use]
    pub fn new() -> Broadcast<T> {
        Broadcast::default()
    }

    pub fn subscribe(&self) -> Subscription<T> {
        let (subscriber, subscription) = subscription();
        self.add(subscriber);
        subscription
    }

    pub fn add(&self, subscriber: Subscriber<T>) {
        self.senders.lock().push(subscriber.0);
    }

    pub fn send(&self, value: &T) {
        self.senders
            .lock()
            .retain(|sender| sender.unbounded_send(value.clone()).is_ok());
    }
}

impl<T> std::fmt::Debug for Broadcast<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Broadcast")
            .field("receivers", &self.senders.lock().len())
            .finish()
    }
}

#[must_use]
pub fn subscription<T>() -> (Subscriber<T>, Subscription<T>) {
    let (sender, receiver) = unbounded();
    (Subscriber(sender), Subscription(receiver))
}

pub struct Subscriber<T>(UnboundedSender<T>);

impl<T> Subscriber<T> {
    pub fn send(&self, value: T) {
        if self.0.unbounded_send(value).is_err() {
            tracing::trace!("subscription already dropped");
        }
    }
}

impl<T> std::fmt::Debug for Subscriber<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Subscriber").finish_non_exhaustive()
    }
}

pub struct Subscription<T>(UnboundedReceiver<T>);

impl<T> Stream for Subscription<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<T>> {
        Pin::new(&mut self.0).poll_next(context)
    }
}

impl<T> std::fmt::Debug for Subscription<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Subscription")
            .finish_non_exhaustive()
    }
}
