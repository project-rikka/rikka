use std::io;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::grant::{Grant, Grantor};
use crate::kit::Kit;

pub fn make_thread_realtime(priority: u32, budget: Duration) -> Result<Grant<u32>> {
    let kernel = match schedule_fifo(priority) {
        Ok(()) => {
            return Ok(Grant {
                level: priority,
                grantor: Grantor::Kernel,
            });
        }
        Err(refused) => refused,
    };
    match ask_kit(priority, budget) {
        Ok(level) => Ok(Grant {
            level,
            grantor: Grantor::RealtimeKit,
        }),
        Err(Failure::Kit(realtime_kit)) => Err(Error::Refused {
            kernel,
            realtime_kit,
        }),
        Err(Failure::Budget(budget)) => Err(Error::Unbudgeted { kernel, budget }),
    }
}

enum Failure {
    Kit(zbus::Error),
    Budget(io::Error),
}

impl From<zbus::Error> for Failure {
    fn from(error: zbus::Error) -> Self {
        Self::Kit(error)
    }
}

fn schedule_fifo(priority: u32) -> io::Result<()> {
    let parameter = libc::sched_param {
        sched_priority: i32::try_from(priority).map_err(|_| io::ErrorKind::InvalidInput)?,
    };
    let policy = libc::SCHED_FIFO | libc::SCHED_RESET_ON_FORK;
    match unsafe { libc::sched_setscheduler(0, policy, &parameter) } {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}

fn ask_kit(priority: u32, budget: Duration) -> std::result::Result<u32, Failure> {
    let kit = Kit::connect()?;
    let ceiling = u32::try_from(kit.max_realtime_priority()?)
        .unwrap_or(1)
        .max(1);
    let wanted = u64::try_from(budget.as_micros()).unwrap_or(u64::MAX);
    let allowed = u64::try_from(kit.rttime_usec_max()?).unwrap_or(wanted);
    limit_realtime(wanted.min(allowed)).map_err(Failure::Budget)?;
    let priority = priority.min(ceiling);
    kit.make_thread_realtime(priority)?;
    Ok(priority)
}

fn limit_realtime(micros: u64) -> io::Result<()> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let known = unsafe { libc::getrlimit(libc::RLIMIT_RTTIME, &mut limit) } == 0;
    if known && limit.rlim_max != libc::RLIM_INFINITY && limit.rlim_max <= micros {
        return Ok(());
    }
    let limit = libc::rlimit {
        rlim_cur: micros,
        rlim_max: micros,
    };
    match unsafe { libc::setrlimit(libc::RLIMIT_RTTIME, &limit) } {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}
