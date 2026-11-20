use std::sync::mpsc::{self, Receiver, SendError, Sender};
use std::sync::{Arc, OnceLock};
use std::task::Wake;

use rikka_worker::{EventFd, EventNotifier};

#[derive(Clone, Default)]
pub struct LoopWaker(Arc<OnceLock<EventNotifier>>);

impl LoopWaker {
    pub fn install(&self) -> std::io::Result<EventFd> {
        let (notifier, wake_fd) = rikka_worker::event_fd()?;
        if self.0.set(notifier).is_err() {
            return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
        }
        Ok(wake_fd)
    }

    pub fn ring(&self) {
        if let Some(notifier) = self.0.get()
            && let Err(error) = notifier.notify()
        {
            tracing::trace!("failed to wake event loop: {error}");
        }
    }

    pub fn channel<T>(&self) -> (WakingSender<T>, Receiver<T>) {
        let (sender, receiver) = mpsc::channel();
        (
            WakingSender {
                sender,
                notifier: self.clone(),
            },
            receiver,
        )
    }
}

impl Wake for LoopWaker {
    fn wake(self: Arc<Self>) {
        self.ring();
    }
}

impl std::fmt::Debug for LoopWaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopWaker")
            .field("hung", &self.0.get().is_some())
            .finish()
    }
}

pub struct WakingSender<T> {
    sender: Sender<T>,
    notifier: LoopWaker,
}

impl<T> WakingSender<T> {
    pub fn post(&self, item: T) -> Result<(), T> {
        self.sender.send(item).map_err(|SendError(item)| item)?;
        self.notifier.ring();
        Ok(())
    }
}

impl<T> Clone for WakingSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            notifier: self.notifier.clone(),
        }
    }
}

impl<T> std::fmt::Debug for WakingSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WakingSender").finish_non_exhaustive()
    }
}
