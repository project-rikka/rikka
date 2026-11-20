use std::sync::Arc;

use futures_util::Stream;
use tokio::sync::watch;

pub struct Observable<T>(Arc<watch::Sender<T>>);

impl<T: Clone + Send + Sync + 'static> Observable<T> {
    pub fn new(value: T) -> Observable<T> {
        Observable(Arc::new(watch::Sender::new(value)))
    }

    pub fn get(&self) -> T {
        self.0.borrow().clone()
    }

    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&self.0.borrow())
    }

    pub fn modify(&self, modify: impl FnOnce(&mut T) -> bool) {
        self.0.send_if_modified(modify);
    }

    pub fn set(&self, value: T) {
        self.0.send_replace(value);
    }

    pub fn receiver_count(&self) -> usize {
        self.0.receiver_count()
    }

    pub fn changes(&self) -> impl Stream<Item = T> + Send + 'static + use<T> {
        let receiver = self.0.subscribe();
        futures_util::stream::unfold(receiver, |mut receiver| async move {
            receiver.changed().await.ok()?;
            let value = receiver.borrow_and_update().clone();
            Some((value, receiver))
        })
    }
}

impl<T: PartialEq + Clone + Send + Sync + 'static> Observable<T> {
    pub fn update(&self, value: T) {
        self.modify(|current| {
            let changed = *current != value;
            if changed {
                *current = value;
            }
            changed
        });
    }
}

impl<T: Default + Clone + Send + Sync + 'static> Default for Observable<T> {
    fn default() -> Self {
        Observable::new(T::default())
    }
}

impl<T> Clone for Observable<T> {
    fn clone(&self) -> Self {
        Observable(Arc::clone(&self.0))
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> std::fmt::Debug for Observable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Observable").field(&self.get()).finish()
    }
}
