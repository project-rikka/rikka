use std::ops::ControlFlow;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use futures_util::{Stream, StreamExt};

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

pub fn drain<S: Stream + Unpin>(mut stream: S, mut each: impl FnMut(S::Item) -> ControlFlow<()>) {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    loop {
        match stream.poll_next_unpin(&mut context) {
            Poll::Ready(Some(item)) => {
                if each(item).is_break() {
                    return;
                }
            }
            Poll::Ready(None) => return,
            Poll::Pending => thread::park(),
        }
    }
}
