use rikka_sync::Mutex;
use std::fmt;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ExitStatus};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;
use rustix::pipe::{PipeFlags, pipe_with};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use crate::process::profile::ProfileFolder;
use crate::threads;

const GRACE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Exit {
    Code(i32),
    Signal(i32),
    Unknown,
}

impl Exit {
    fn from_status(status: io::Result<ExitStatus>) -> Exit {
        match status {
            Ok(status) => match (status.code(), status.signal()) {
                (Some(code), _) => Exit::Code(code),
                (None, Some(signal)) => Exit::Signal(signal),
                (None, None) => Exit::Unknown,
            },
            Err(error) => {
                tracing::warn!(%error, "failed to reap the engine process");
                Exit::Unknown
            }
        }
    }
}

type Teardown = Box<dyn FnOnce() + Send>;

#[derive(Default)]
enum Parting {
    #[default]
    Watching,
    Left(Teardown),
    Reaped,
}

static CLOSING: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

pub(crate) struct Watch {
    stop: OwnedFd,
    thread: JoinHandle<()>,
    parting: Arc<Mutex<Parting>>,
}

impl fmt::Debug for Watch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Watch").finish_non_exhaustive()
    }
}

impl Watch {
    pub(crate) fn part(self, teardown: impl FnOnce() + Send + 'static) {
        let teardown: Teardown = Box::new(teardown);
        let late = {
            let mut parting = self.parting.lock();
            match std::mem::take(&mut *parting) {
                Parting::Reaped => {
                    *parting = Parting::Reaped;
                    Some(teardown)
                }
                Parting::Watching | Parting::Left(_) => {
                    *parting = Parting::Left(teardown);
                    None
                }
            }
        };
        drop(self.stop);
        match late {
            Some(teardown) => {
                threads::join(self.thread);
                teardown();
            }
            None => {
                let mut closing = CLOSING.lock();
                closing.retain(|thread| !thread.is_finished());
                closing.push(self.thread);
            }
        }
    }
}

pub fn wait_for_closed_runtimes() {
    let closing = std::mem::take(&mut *CLOSING.lock());
    for thread in closing {
        threads::join(thread);
    }
}

pub(crate) fn watch(
    mut child: Child,
    folder: ProfileFolder,
    exited: impl FnOnce(Exit) + Send + 'static,
) -> io::Result<Watch> {
    let (stopped, stop) = pipe_with(PipeFlags::CLOEXEC)?;
    let process = match pidfd_open(Pid::from_child(&child), PidfdFlags::empty()) {
        Ok(process) => process,
        Err(error) => {
            kill(&mut child);
            if let Err(error) = child.wait() {
                tracing::warn!(%error, "failed to reap the engine process");
            }
            return Err(error.into());
        }
    };
    let parting = Arc::new(Mutex::new(Parting::Watching));
    let left = Arc::clone(&parting);
    let thread = std::thread::Builder::new()
        .name("rikka-gecko-watch".into())
        .spawn(move || {
            if !ended(&process, &stopped) && !wait_for_exit(&process, GRACE) {
                kill(&mut child);
            }
            let status = child.wait();
            drop(folder);
            exited(Exit::from_status(status));
            let teardown = std::mem::replace(&mut *left.lock(), Parting::Reaped);
            if let Parting::Left(teardown) = teardown {
                teardown();
            }
        })?;
    Ok(Watch {
        stop,
        thread,
        parting,
    })
}

fn kill(child: &mut Child) {
    if let Err(error) = child.kill() {
        tracing::warn!(%error, "failed to kill the engine process");
    }
}

fn ended(process: &OwnedFd, stopped: &OwnedFd) -> bool {
    let mut watched = [
        PollFd::new(process, PollFlags::IN),
        PollFd::new(stopped, PollFlags::IN),
    ];
    loop {
        match poll(&mut watched, None) {
            Err(Errno::INTR) => {}
            Err(error) => {
                tracing::warn!(%error, "failed to poll the engine process");
                return false;
            }
            Ok(_) => return !watched[0].revents().is_empty(),
        }
    }
}

fn wait_for_exit(process: &OwnedFd, timeout: Duration) -> bool {
    let mut watched = [PollFd::new(process, PollFlags::IN)];
    let timeout = Timespec::try_from(timeout).expect("grace period fits a timespec");
    loop {
        match poll(&mut watched, Some(&timeout)) {
            Err(Errno::INTR) => {}
            Err(error) => {
                tracing::warn!(%error, "failed to poll the engine process");
                return false;
            }
            Ok(ready) => return ready > 0,
        }
    }
}
