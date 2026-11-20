use std::time::Duration;

use futures_channel::mpsc::UnboundedReceiver;
use futures_util::StreamExt;

pub(crate) const PATIENCE: Duration = Duration::from_secs(10);

pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    block_on_within(PATIENCE, future)
}

pub(crate) fn block_on_within<F: Future>(timeout: Duration, future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("a runtime for the test")
        .block_on(async { tokio::time::timeout(timeout, future).await })
        .expect("the future settles in time")
}

pub(crate) fn next<T>(events: &mut UnboundedReceiver<T>) -> T {
    block_on(events.next()).expect("the stream yields another event")
}

pub(crate) fn next_matching<T, U>(
    events: &mut UnboundedReceiver<T>,
    pick: impl FnMut(T) -> Option<U>,
) -> U {
    next_matching_within(PATIENCE, events, pick)
}

pub(crate) fn next_matching_within<T, U>(
    timeout: Duration,
    events: &mut UnboundedReceiver<T>,
    mut pick: impl FnMut(T) -> Option<U>,
) -> U {
    block_on_within(timeout, async {
        loop {
            let event = events
                .next()
                .await
                .expect("the stream yields another event");
            if let Some(picked) = pick(event) {
                return picked;
            }
        }
    })
}
