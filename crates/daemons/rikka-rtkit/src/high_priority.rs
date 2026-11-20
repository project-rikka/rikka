use std::io;

use crate::error::{Error, Result};
use crate::grant::{Grant, Grantor};
use crate::kit::Kit;

pub fn make_thread_high_priority(nice: i32) -> Result<Grant<i32>> {
    let kernel = match renice(nice) {
        Ok(()) => {
            return Ok(Grant {
                level: nice,
                grantor: Grantor::Kernel,
            });
        }
        Err(refused) => refused,
    };
    match ask_kit(nice) {
        Ok(level) => Ok(Grant {
            level,
            grantor: Grantor::RealtimeKit,
        }),
        Err(realtime_kit) => Err(Error::Refused {
            kernel,
            realtime_kit,
        }),
    }
}

fn renice(nice: i32) -> io::Result<()> {
    match unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) } {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}

fn ask_kit(nice: i32) -> zbus::Result<i32> {
    let kit = Kit::connect()?;
    let nice = nice.max(kit.min_nice_level()?);
    kit.make_thread_high_priority(nice)?;
    Ok(nice)
}
