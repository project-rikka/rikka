use std::os::fd::OwnedFd;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;
use rustix::process::{Pid, PidfdFlags, Signal, kill_process_group, pidfd_open};

pub(crate) const PATIENCE: Duration = Duration::from_secs(30);

pub(crate) fn succeeded(command: &mut Command, patience: Duration) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    let leader = Pid::from_child(&child);

    let Ok(pidfd) = pidfd_open(leader, PidfdFlags::empty()) else {
        return child.wait().is_ok_and(|status| status.success());
    };
    let ended = ended_within(&pidfd, patience);
    kill_process_group(leader, Signal::KILL).ok();
    let status = child.wait();
    ended && status.is_ok_and(|status| status.success())
}

fn ended_within(pidfd: &OwnedFd, patience: Duration) -> bool {
    let deadline = Instant::now() + patience;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let timeout = Timespec {
            tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut watched = [PollFd::new(pidfd, PollFlags::IN)];
        match poll(&mut watched, Some(&timeout)) {
            Ok(0) => return false,
            Ok(_) => return true,
            Err(Errno::INTR) => {}
            Err(_) => return false,
        }
    }
}
