use std::fmt::Debug;
use std::os::fd::{AsFd as _, BorrowedFd};
use std::time::{Duration, Instant};

use rikka_wayland_client::Queue;
use rustix::event::{PollFd, PollFlags, Timespec, poll};

pub fn dispatch_until<S: Debug>(
    queue: &mut Queue<S>,
    state: &mut S,
    what: &str,
    patience: Duration,
    done: impl Fn(&S) -> bool,
) {
    let deadline = Instant::now() + patience;
    queue.dispatch_pending(state).expect("the display stays up");
    while !done(state) {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}; state: {state:?}"
        );
        dispatch_next(queue, state, &[], deadline);
    }
}

pub fn dispatch_next<S>(
    queue: &mut Queue<S>,
    state: &mut S,
    also: &[BorrowedFd<'_>],
    deadline: Instant,
) -> Vec<bool> {
    let connection = queue.connection().clone();
    connection.flush().expect("the display reads requests");
    if queue.has_pending() {
        queue.dispatch_pending(state).expect("the display stays up");
        return vec![false; also.len()];
    }
    let mut ready = wait_readable(
        std::iter::once(connection.as_fd()).chain(also.iter().copied()),
        deadline,
    );
    if ready.remove(0) {
        connection
            .read()
            .expect("the display sends readable events");
    }
    queue.dispatch_pending(state).expect("the display stays up");
    ready
}

pub fn wait_readable<'fd>(
    fds: impl IntoIterator<Item = BorrowedFd<'fd>>,
    deadline: Instant,
) -> Vec<bool> {
    let mut polled: Vec<PollFd<'fd>> = fds
        .into_iter()
        .map(|fd| PollFd::from_borrowed_fd(fd, PollFlags::IN))
        .collect();
    let remaining = deadline.saturating_duration_since(Instant::now());
    let timeout = Timespec::try_from(remaining).expect("a representable timeout");
    match poll(&mut polled, Some(&timeout)) {
        Ok(_) | Err(rustix::io::Errno::INTR) => {}
        Err(error) => panic!("poll failed: {error}"),
    }
    polled
        .iter()
        .map(|fd| {
            fd.revents()
                .intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR)
        })
        .collect()
}
