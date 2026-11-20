use std::future::Future;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Wake, Waker};

use rikka_types::{CancelToken, Cancelled};
use rustix::pipe::{PipeFlags, pipe_with};

pub(crate) struct CancelSignal<'a> {
    _cancelled: Pin<Box<Cancelled<'a>>>,
    readable: OwnedFd,
}

impl<'a> CancelSignal<'a> {
    pub(crate) fn new(token: &'a CancelToken) -> io::Result<CancelSignal<'a>> {
        let (readable, writable) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)?;
        let notifier = Arc::new(PipeNotifier(writable));
        let mut cancelled = Box::pin(token.cancelled());
        if cancelled
            .as_mut()
            .poll(&mut Context::from_waker(&Waker::from(Arc::clone(
                &notifier,
            ))))
            .is_ready()
        {
            notifier.notify();
        }
        Ok(CancelSignal {
            _cancelled: cancelled,
            readable,
        })
    }
}

impl AsFd for CancelSignal<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.readable.as_fd()
    }
}

struct PipeNotifier(OwnedFd);

impl PipeNotifier {
    fn notify(&self) {
        if let Err(error) = rustix::io::write(&self.0, &[1]) {
            tracing::debug!(%error, "failed to signal cancellation");
        }
    }
}

impl Wake for PipeNotifier {
    fn wake(self: Arc<Self>) {
        self.notify();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.notify();
    }
}
