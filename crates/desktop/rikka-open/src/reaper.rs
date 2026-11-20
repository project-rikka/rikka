use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::process::Child;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender};

use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

const NAME: &str = "rikka-open-reaper";

struct Reaper {
    arriving: Sender<Child>,
    bell: OwnedFd,
}

pub(crate) fn reap(child: Child) {
    static REAPER: OnceLock<Option<Reaper>> = OnceLock::new();
    let reaper = REAPER.get_or_init(|| {
        start()
            .inspect_err(|error| tracing::warn!(%error, "failed to start child reaper"))
            .ok()
    });
    let Some(reaper) = reaper else {
        return;
    };
    if reaper.arriving.send(child).is_ok() {
        let _ = rustix::io::write(&reaper.bell, &1_u64.to_ne_bytes());
    }
}

fn start() -> io::Result<Reaper> {
    let bell = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?;
    let ringing = bell.try_clone()?;
    let (arriving, arrived) = mpsc::channel();
    std::thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || watch(&ringing, &arrived))?;
    Ok(Reaper { arriving, bell })
}

fn watch(bell: &OwnedFd, arrived: &Receiver<Child>) {
    let mut children: Vec<(Child, OwnedFd)> = Vec::new();
    loop {
        let ready: Vec<bool> = {
            let mut ends: Vec<PollFd<'_>> = std::iter::once(bell.as_fd())
                .chain(children.iter().map(|(_, ended)| ended.as_fd()))
                .map(|fd| PollFd::from_borrowed_fd(fd, PollFlags::IN))
                .collect();
            match poll(&mut ends, None) {
                Ok(_) => ends.iter().map(|end| !end.revents().is_empty()).collect(),
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => {
                    tracing::warn!(%error, "child reaper stopped");
                    return;
                }
            }
        };
        let mut at = 0;
        children.retain_mut(|(child, _)| {
            at += 1;
            !ready[at] || matches!(child.try_wait(), Ok(None))
        });
        if ready[0] {
            let mut count = [0_u8; 8];
            let _ = rustix::io::read(bell, &mut count);
            children.extend(arrived.try_iter().filter_map(with_pidfd));
        }
    }
}

fn with_pidfd(mut child: Child) -> Option<(Child, OwnedFd)> {
    match pidfd_open(Pid::from_child(&child), PidfdFlags::empty()) {
        Ok(ended) => Some((child, ended)),
        Err(error) => {
            tracing::debug!(%error, "reaper unavailable, waiting on a dedicated thread");
            let waiting = std::thread::Builder::new()
                .name(NAME.to_owned())
                .spawn(move || drop(child.wait()));
            if let Err(error) = waiting {
                tracing::warn!(%error, "failed to spawn wait thread");
            }
            None
        }
    }
}
