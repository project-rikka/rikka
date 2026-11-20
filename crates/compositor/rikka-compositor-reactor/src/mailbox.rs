use std::fmt;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use rustix::event::{EventfdFlags, eventfd};

#[derive(Debug)]
pub(crate) struct Notifier {
    fd: OwnedFd,
    open: AtomicBool,
}

impl Notifier {
    pub(crate) fn new() -> std::io::Result<Arc<Self>> {
        let fd = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?;
        Ok(Arc::new(Self {
            fd,
            open: AtomicBool::new(true),
        }))
    }

    pub(crate) fn fd(&self) -> &OwnedFd {
        &self.fd
    }

    pub(crate) fn ring(&self) {
        let _ = rustix::io::write(&self.fd, &1u64.to_ne_bytes());
    }

    pub(crate) fn reset(&self) {
        let mut count = [0u8; 8];
        let _ = rustix::io::read(self.fd.as_fd(), &mut count);
    }

    pub(crate) fn close(&self) {
        self.open.store(false, Ordering::Release);
    }
}

pub struct Sender<M> {
    channel: mpsc::Sender<M>,
    notifier: Arc<Notifier>,
}

impl<M> Sender<M> {
    pub(crate) fn new(channel: mpsc::Sender<M>, notifier: Arc<Notifier>) -> Self {
        Self { channel, notifier }
    }

    pub fn send(&self, message: M) -> Result<(), Closed<M>> {
        if !self.notifier.open.load(Ordering::Acquire) {
            return Err(Closed(message));
        }
        self.channel
            .send(message)
            .map_err(|mpsc::SendError(message)| Closed(message))?;
        self.notifier.ring();
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.notifier.open.load(Ordering::Acquire)
    }
}

impl<M> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Self {
            channel: self.channel.clone(),
            notifier: self.notifier.clone(),
        }
    }
}

impl<M> fmt::Debug for Sender<M> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sender")
            .field("open", &self.is_open())
            .finish()
    }
}

#[derive(thiserror::Error)]
#[error("mailbox was removed from its reactor")]
pub struct Closed<M>(pub M);

impl<M> fmt::Debug for Closed<M> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Closed(..)")
    }
}
