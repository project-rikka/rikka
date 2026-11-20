use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, Command};
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, poll};
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
use rustix::time::Timespec;

const STOP_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct Supervised {
    child: Child,
}

impl Supervised {
    pub(crate) fn spawn(command: &mut Command) -> io::Result<Supervised> {
        unsafe {
            command.pre_exec(|| {
                rustix::process::set_parent_process_death_signal(Some(Signal::KILL))
                    .map_err(io::Error::from)
            });
        }
        Ok(Supervised {
            child: command.spawn()?,
        })
    }

    #[must_use]
    pub(crate) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub(crate) fn has_exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    fn terminate(&self) -> io::Result<bool> {
        let pidfd = pidfd_open(Pid::from_child(&self.child), PidfdFlags::empty())?;
        pidfd_send_signal(&pidfd, Signal::TERM)?;
        let timeout = Timespec::try_from(STOP_TIMEOUT).map_err(io::Error::other)?;
        let mut exit = [PollFd::new(&pidfd, PollFlags::IN)];
        Ok(poll(&mut exit, Some(&timeout))? > 0)
    }
}

impl Drop for Supervised {
    fn drop(&mut self) {
        let pid = self.child.id();
        if !self.has_exited() {
            let terminated = self.terminate().unwrap_or_else(|error| {
                eprintln!("failed to terminate {pid}: {error}");
                false
            });
            if !terminated && let Err(error) = self.child.kill() {
                eprintln!("failed to kill {pid}: {error}");
            }
        }
        if let Err(error) = self.child.wait() {
            eprintln!("failed to reap {pid}: {error}");
        }
    }
}

impl std::fmt::Debug for Supervised {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Supervised")
            .field("pid", &self.child.id())
            .finish()
    }
}
