#[cfg(unix)]
use std::os::fd::{AsFd, BorrowedFd};
#[cfg(windows)]
use std::os::windows::io::{AsHandle, BorrowedHandle};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::task::Waker;
use std::time::Duration;

use crate::error::SendError;
use crate::event_fd::{EventFd, EventNotifier, event_fd};
use crate::worker::Stop;

pub type Reply<R> = SyncSender<R>;

pub struct CommandSender<C> {
    commands: Sender<C>,
    notifier: EventNotifier,
    stopped: Arc<AtomicBool>,
    stop_command: Box<dyn Fn() -> C + Send + Sync>,
}

pub struct CommandReceiver<C> {
    commands: Receiver<C>,
    event_fd: EventFd,
    notifier: EventNotifier,
    stopped: Arc<AtomicBool>,
}

pub fn command_channel<C>(
    stop_command: impl Fn() -> C + Send + Sync + 'static,
) -> std::io::Result<(CommandSender<C>, CommandReceiver<C>)> {
    let (notifier, event_fd) = event_fd()?;
    let (commands, receiver) = mpsc::channel();
    let stopped = Arc::new(AtomicBool::new(false));
    Ok((
        CommandSender {
            commands,
            notifier: notifier.clone(),
            stopped: Arc::clone(&stopped),
            stop_command: Box::new(stop_command),
        },
        CommandReceiver {
            commands: receiver,
            event_fd,
            notifier,
            stopped,
        },
    ))
}

impl<C> CommandSender<C> {
    pub fn send(&self, command: C) -> Result<(), SendError> {
        self.commands
            .send(command)
            .map_err(|_| SendError::Disconnected)?;
        self.notifier.notify().map_err(SendError::Io)
    }

    pub fn request<R>(
        &self,
        command: impl FnOnce(Reply<R>) -> C,
        timeout: Duration,
    ) -> Result<R, SendError> {
        let (reply, response) = mpsc::sync_channel(1);
        self.send(command(reply))?;
        response.recv_timeout(timeout).map_err(|error| match error {
            RecvTimeoutError::Timeout => SendError::Timeout,
            RecvTimeoutError::Disconnected => SendError::Disconnected,
        })
    }
}

impl<C: 'static> Stop for CommandSender<C> {
    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Err(error) = self.send((self.stop_command)()) {
            tracing::debug!(%error, "failed to send stop command");
        }
    }
}

impl<C> CommandReceiver<C> {
    pub fn drain(&self) -> impl Iterator<Item = C> + '_ {
        self.event_fd.drain();
        self.commands.try_iter()
    }

    pub fn wait(&self) -> std::io::Result<()> {
        self.event_fd.wait()
    }

    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    #[cfg(unix)]
    pub fn wait_readable(
        &self,
        fd: BorrowedFd<'_>,
        timeout: impl Into<Option<Duration>>,
    ) -> std::io::Result<bool> {
        self.event_fd.wait_readable(fd, timeout)
    }

    #[cfg(windows)]
    pub fn wait_signaled(
        &self,
        handle: BorrowedHandle<'_>,
        timeout: impl Into<Option<Duration>>,
    ) -> std::io::Result<bool> {
        self.event_fd
            .wait_signaled([handle], timeout)
            .map(|[signaled]| signaled)
    }

    #[must_use]
    pub fn waker(&self) -> Waker {
        Waker::from(Arc::new(self.notifier.clone()))
    }
}

#[cfg(unix)]
impl<C> AsFd for CommandReceiver<C> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.event_fd.as_fd()
    }
}

#[cfg(windows)]
impl<C> AsHandle for CommandReceiver<C> {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.event_fd.as_handle()
    }
}

impl<C> std::fmt::Debug for CommandSender<C> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommandSender")
            .finish_non_exhaustive()
    }
}

impl<C> std::fmt::Debug for CommandReceiver<C> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommandReceiver")
            .finish_non_exhaustive()
    }
}
