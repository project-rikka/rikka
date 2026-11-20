use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;
use std::task::Wake;
use std::time::{Duration, Instant};

use rustix::event::{EventfdFlags, PollFd, PollFlags, Timespec, eventfd, poll};
use rustix::io::Errno;

#[derive(Clone)]
pub struct EventNotifier {
    fd: Arc<OwnedFd>,
}

pub struct EventFd {
    fd: Arc<OwnedFd>,
}

pub fn event_fd() -> std::io::Result<(EventNotifier, EventFd)> {
    let fd = Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?);
    Ok((
        EventNotifier {
            fd: Arc::clone(&fd),
        },
        EventFd { fd },
    ))
}

impl EventNotifier {
    pub fn notify(&self) -> std::io::Result<()> {
        match rustix::io::write(&self.fd, &1_u64.to_ne_bytes()) {
            Ok(_) | Err(Errno::AGAIN) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl Wake for EventNotifier {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if let Err(error) = self.notify() {
            tracing::debug!(%error, "failed to signal eventfd");
        }
    }
}

impl AsFd for EventFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl EventFd {
    pub fn drain(&self) {
        let mut count = [0_u8; 8];
        let _ = rustix::io::read(&self.fd, &mut count);
    }

    pub fn wait(&self) -> std::io::Result<()> {
        self.poll_readable([], None).map(drop)
    }

    pub fn wait_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        self.poll_readable([], timeout).map(drop)
    }

    pub fn wait_readable(
        &self,
        fd: BorrowedFd<'_>,
        timeout: impl Into<Option<Duration>>,
    ) -> std::io::Result<bool> {
        self.poll_readable([fd], timeout).map(|[ready]| ready)
    }

    pub fn poll_readable<const N: usize>(
        &self,
        fds: [BorrowedFd<'_>; N],
        timeout: impl Into<Option<Duration>>,
    ) -> std::io::Result<[bool; N]> {
        let deadline = timeout.into().map(|timeout| Instant::now() + timeout);
        let mut polled: Vec<PollFd<'_>> = fds
            .iter()
            .map(|fd| PollFd::from_borrowed_fd(*fd, PollFlags::IN))
            .chain(std::iter::once(PollFd::new(&self.fd, PollFlags::IN)))
            .collect();
        match poll_until(&mut polled, deadline)? {
            true => Ok(std::array::from_fn(|index| {
                !polled[index].revents().is_empty()
            })),
            false => Ok([false; N]),
        }
    }
}

pub fn wait_readable(fd: BorrowedFd<'_>, deadline: Option<Instant>) -> std::io::Result<bool> {
    poll_until(&mut [PollFd::from_borrowed_fd(fd, PollFlags::IN)], deadline)
}

fn poll_until(fds: &mut [PollFd<'_>], deadline: Option<Instant>) -> std::io::Result<bool> {
    loop {
        let remaining = deadline.map(|deadline| {
            let left = deadline.saturating_duration_since(Instant::now());
            Timespec::try_from(left).unwrap_or(Timespec {
                tv_sec: i64::MAX,
                tv_nsec: 0,
            })
        });
        match poll(fds, remaining.as_ref()) {
            Ok(ready) => return Ok(ready > 0),
            Err(Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }
    }
}

impl std::fmt::Debug for EventNotifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EventNotifier")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for EventFd {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("EventFd").finish_non_exhaustive()
    }
}
