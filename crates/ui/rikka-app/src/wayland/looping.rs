use std::os::fd::AsFd as _;
use std::time::Instant;

use rikka_wayland_client::Queue;
use rikka_worker::EventFd;

use crate::error::{Error, Result};
use crate::tasks::LoopReceiver;
use crate::wayland::runner::Runner;

pub(crate) fn run(
    runner: &mut Runner,
    mut queue: Queue<Runner>,
    mailbox: &LoopReceiver,
    wake_fd: EventFd,
) -> Result<()> {
    runner.start(&mut queue)?;
    while !runner.exiting() {
        if let Err(error) = queue.dispatch_pending(runner) {
            runner.fail(connection_error(error));
            break;
        }
        while let Some(event) = mailbox.take() {
            runner.user_event(event);
        }
        let deadline = runner.turn();
        if runner.exiting() {
            break;
        }
        if let Err(error) = runner.client.connection.flush() {
            runner.fail(connection_error(error));
            break;
        }
        if queue.has_pending() {
            continue;
        }
        let left = deadline.map(|due| due.saturating_duration_since(Instant::now()));
        let [display] = wake_fd.poll_readable([runner.client.connection.as_fd()], left)?;
        wake_fd.drain();
        if !display {
            continue;
        }
        if let Err(error) = runner.client.connection.read() {
            runner.fail(connection_error(error));
            break;
        }
    }
    Ok(())
}

fn connection_error(error: rikka_wayland_client::Error) -> Error {
    Error::Display(error.into())
}
