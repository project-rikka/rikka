use rikka_sync::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rikka_llm::Usage;
use tokio::sync::Notify;

#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    text: String,
    usage: Usage,
    wakes: bool,
}

impl Notice {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Notice {
        Notice {
            text: text.into(),
            usage: Usage::default(),
            wakes: true,
        }
    }

    #[must_use]
    pub fn with_usage(mut self, usage: Usage) -> Self {
        self.usage = usage;
        self
    }

    #[must_use]
    pub fn held(mut self) -> Self {
        self.wakes = false;
        self
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn usage(&self) -> Usage {
        self.usage
    }

    #[must_use]
    pub fn wakes(&self) -> bool {
        self.wakes
    }
}

#[derive(Clone, Debug, Default)]
pub struct Notices {
    inner: Arc<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    posted: Mutex<VecDeque<Notice>>,
    outstanding: AtomicUsize,
    arrived: Notify,
}

impl Notices {
    #[must_use]
    pub fn new() -> Notices {
        Notices::default()
    }

    pub fn pending(&self) -> PendingNotice {
        self.inner.outstanding.fetch_add(1, Ordering::AcqRel);
        PendingNotice {
            notices: self.clone(),
        }
    }

    pub fn post(&self, notice: Notice) {
        self.inner.posted.lock().push_back(notice);
        self.inner.arrived.notify_waiters();
    }

    #[must_use]
    pub fn take(&self) -> Vec<Notice> {
        self.inner.posted.lock().drain(..).collect()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.posted.lock().is_empty()
    }

    #[must_use]
    pub fn is_waking(&self) -> bool {
        self.inner.posted.lock().iter().any(Notice::wakes)
    }

    pub fn hold(&self) {
        for notice in self.inner.posted.lock().iter_mut() {
            notice.wakes = false;
        }
    }

    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.inner.outstanding.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.outstanding() == 0 && self.is_empty()
    }

    pub async fn wait(&self) -> Vec<Notice> {
        loop {
            let waiting = self.inner.arrived.notified();
            tokio::pin!(waiting);
            waiting.as_mut().enable();

            let posted = self.take();
            if !posted.is_empty() || self.outstanding() == 0 {
                return posted;
            }

            waiting.await;
        }
    }

    fn released(&self) {
        self.inner.outstanding.fetch_sub(1, Ordering::AcqRel);
        self.inner.arrived.notify_waiters();
    }
}

#[must_use = "the notice is outstanding only while this guard is held"]
#[derive(Debug)]
pub struct PendingNotice {
    notices: Notices,
}

impl PendingNotice {
    pub fn post(self, notice: Notice) {
        self.notices.post(notice);
    }
}

impl Drop for PendingNotice {
    fn drop(&mut self) {
        self.notices.released();
    }
}
