use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use rikka_types::CancelToken;

#[derive(Debug, Default)]
struct CountingWaker(AtomicUsize);

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn cancel_before_poll_resolves_immediately() {
    let token = CancelToken::new();
    token.cancel();

    let mut cancelled = token.cancelled();
    let poll = Pin::new(&mut cancelled).poll(&mut Context::from_waker(Waker::noop()));

    assert_eq!(poll, Poll::Ready(()));
}

#[test]
fn cancel_after_poll_wakes_all_waiters() {
    let token = CancelToken::new();
    let counters: Vec<_> = (0..3).map(|_| Arc::new(CountingWaker::default())).collect();
    let wakers: Vec<_> = counters
        .iter()
        .map(|counter| Waker::from(Arc::clone(counter)))
        .collect();
    let mut futures: Vec<_> = wakers.iter().map(|_| token.cancelled()).collect();
    for (future, waker) in futures.iter_mut().zip(&wakers) {
        assert_eq!(
            Pin::new(future).poll(&mut Context::from_waker(waker)),
            Poll::Pending
        );
    }
    let gone = futures.pop();
    drop(gone);

    token.cancel();

    let wakes: Vec<_> = counters
        .iter()
        .map(|counter| counter.0.load(Ordering::SeqCst))
        .collect();
    assert_eq!(wakes, [1, 1, 0], "a waiter that is gone is not woken");
    for (future, waker) in futures.iter_mut().zip(&wakers) {
        assert_eq!(
            Pin::new(future).poll(&mut Context::from_waker(waker)),
            Poll::Ready(())
        );
    }
}

#[test]
fn wait_timeout_wakes_on_cancel_from_another_thread() {
    let token = CancelToken::new();
    let canceller = token.clone();
    let started = Instant::now();
    let thread = std::thread::spawn(move || canceller.cancel());

    assert!(token.wait_timeout(Duration::from_secs(30)));
    assert!(started.elapsed() < Duration::from_secs(30));
    thread.join().expect("canceller thread finishes");
}

#[test]
fn wait_timeout_expires_without_cancel() {
    let token = CancelToken::new();
    assert!(!token.wait_timeout(Duration::from_millis(1)));
}

#[test]
fn child_follows_parent_but_not_reverse() {
    let parent = CancelToken::new();
    let child = parent.child();
    let sibling = parent.child();

    child.cancel();
    assert!(!parent.is_cancelled());
    assert!(!sibling.is_cancelled());

    parent.cancel();
    assert!(sibling.is_cancelled());
    assert!(parent.child().is_cancelled());
}

#[test]
fn listeners_hear_cancel_once_even_when_late() {
    let token = CancelToken::new();
    let heard = Arc::new(AtomicUsize::new(0));
    let early = Arc::clone(&heard);
    token.on_cancel(move || {
        early.fetch_add(1, Ordering::SeqCst);
    });
    token.cancel();
    token.cancel();
    let late = Arc::clone(&heard);
    token.on_cancel(move || {
        late.fetch_add(1, Ordering::SeqCst);
    });

    assert_eq!(heard.load(Ordering::SeqCst), 2);
}
