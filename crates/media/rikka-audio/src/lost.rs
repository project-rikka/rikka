use rikka_sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use crate::stream::StreamShared;

#[derive(Debug, Default)]
pub(crate) struct DeviceLost {
    lost: AtomicBool,
    feeds: Mutex<Vec<Weak<StreamShared>>>,
}

impl DeviceLost {
    pub(crate) fn mark(&self) {
        self.lost.store(true, Ordering::Release);
        for feed in self.feeds.lock().iter().filter_map(Weak::upgrade) {
            feed.notify_producer();
        }
    }

    pub(crate) fn is_lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }

    pub(crate) fn wake_on_loss(&self, feed: &Arc<StreamShared>) {
        let mut feeds = self.feeds.lock();
        feeds.retain(|known| known.strong_count() > 0);
        feeds.push(Arc::downgrade(feed));
        drop(feeds);
        if self.is_lost() {
            feed.notify_producer();
        }
    }
}
