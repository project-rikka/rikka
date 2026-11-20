use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<Shared>);

#[derive(Debug, Default)]
struct Shared {
    cancelled: AtomicBool,
    waiters: Mutex<Waiters>,
    signal: Condvar,
}

#[derive(Debug, Default)]
struct Waiters {
    wakers: Vec<Option<Waker>>,
    children: Vec<Weak<Shared>>,
    listeners: Listeners,
}

#[derive(Default)]
struct Listeners(Vec<Box<dyn FnOnce() + Send>>);

impl std::fmt::Debug for Listeners {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Listeners").field(&self.0.len()).finish()
    }
}

impl CancelToken {
    #[must_use]
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    #[must_use]
    pub fn child(&self) -> CancelToken {
        let child = CancelToken::new();
        let mut waiters = self.0.lock();
        if self.is_cancelled() {
            drop(waiters);
            child.cancel();
            return child;
        }
        waiters.children.retain(|child| child.strong_count() > 0);
        waiters.children.push(Arc::downgrade(&child.0));
        child
    }

    pub fn cancel(&self) {
        self.0.cancel();
    }

    pub fn on_cancel(&self, listener: impl FnOnce() + Send + 'static) {
        let mut waiters = self.0.lock();
        if self.is_cancelled() {
            drop(waiters);
            listener();
            return;
        }
        waiters.listeners.0.push(Box::new(listener));
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }

    pub fn cancelled(&self) -> Cancelled<'_> {
        Cancelled {
            shared: &self.0,
            slot: None,
        }
    }

    #[must_use]
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        let waiters = self.0.lock();
        let (_waiters, _) = self
            .0
            .signal
            .wait_timeout_while(waiters, timeout, |_| !self.is_cancelled());
        self.is_cancelled()
    }
}

impl Shared {
    fn cancel(&self) {
        if self.cancelled.swap(true, Ordering::SeqCst) {
            return;
        }
        let Waiters {
            wakers,
            children,
            listeners,
        } = std::mem::take(&mut *self.lock());
        self.signal.notify_all();
        for waker in wakers.into_iter().flatten() {
            waker.wake();
        }
        for listener in listeners.0 {
            listener();
        }
        for child in children.iter().filter_map(Weak::upgrade) {
            child.cancel();
        }
    }

    fn lock(&self) -> MutexGuard<'_, Waiters> {
        self.waiters.lock()
    }
}

#[derive(Debug)]
pub struct Cancelled<'a> {
    shared: &'a Shared,
    slot: Option<usize>,
}

impl Future for Cancelled<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.shared.cancelled.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }

        let this = self.get_mut();
        let mut waiters = this.shared.lock();
        if this.shared.cancelled.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        let waker = context.waker().clone();
        let wakers = &mut waiters.wakers;
        match this.slot {
            Some(index) => wakers[index] = Some(waker),
            None => {
                let index = match wakers.iter().position(Option::is_none) {
                    Some(free) => free,
                    None => {
                        wakers.push(None);
                        wakers.len() - 1
                    }
                };
                wakers[index] = Some(waker);
                this.slot = Some(index);
            }
        }
        Poll::Pending
    }
}

impl Drop for Cancelled<'_> {
    fn drop(&mut self) {
        let Some(index) = self.slot else {
            return;
        };
        if let Some(slot) = self.shared.lock().wakers.get_mut(index) {
            *slot = None;
        }
    }
}
