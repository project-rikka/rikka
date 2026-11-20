use rustix::io::Errno;
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};

use crate::error::{Error, Result};
use crate::process::Process;
use crate::stat::Stat;

pub fn terminate(process: &Process) -> Result<()> {
    send(process, Signal::TERM)
}

pub fn kill(process: &Process) -> Result<()> {
    send(process, Signal::KILL)
}

fn send(process: &Process, signal: Signal) -> Result<()> {
    let pid = process.pid;
    let Some(id) = Pid::from_raw(pid) else {
        return Err(Error::NoSuchProcess(pid));
    };
    let handle = pidfd_open(id, PidfdFlags::empty()).map_err(|errno| error(pid, errno))?;
    if Stat::read(pid).is_none_or(|stat| stat.start_time != process.start_time) {
        return Err(Error::NoSuchProcess(pid));
    }
    pidfd_send_signal(&handle, signal).map_err(|errno| error(pid, errno))
}

fn error(pid: i32, errno: Errno) -> Error {
    match errno {
        Errno::SRCH => Error::NoSuchProcess(pid),
        Errno::PERM => Error::PermissionDenied(pid),
        _ => Error::Signal {
            pid,
            source: errno.into(),
        },
    }
}
