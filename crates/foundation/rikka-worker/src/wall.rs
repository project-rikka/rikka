use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::time::{Duration, SystemTime};

use rustix::io::Errno;
use rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};

const MIN_EXPIRY: Duration = Duration::from_nanos(1);
const NEVER: Timespec = Timespec {
    tv_sec: 0,
    tv_nsec: 0,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expiry {
    Elapsed,
    ClockChanged,
}

pub struct WallAlarm {
    fd: OwnedFd,
}

impl WallAlarm {
    pub fn new() -> io::Result<WallAlarm> {
        let fd = timerfd_create(
            TimerfdClockId::Realtime,
            TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
        )?;
        Ok(WallAlarm { fd })
    }

    pub fn arm(&self, at: SystemTime) -> io::Result<()> {
        let since = at
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .max(MIN_EXPIRY);
        let value = Timespec::try_from(since)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        self.set(
            TimerfdTimerFlags::ABSTIME | TimerfdTimerFlags::CANCEL_ON_SET,
            value,
        )
    }

    pub fn disarm(&self) -> io::Result<()> {
        self.set(TimerfdTimerFlags::empty(), NEVER)
    }

    pub fn read(&self) -> io::Result<Option<Expiry>> {
        let mut expirations = [0_u8; 8];
        match rustix::io::read(&self.fd, &mut expirations) {
            Ok(_) => Ok(Some(Expiry::Elapsed)),
            Err(Errno::CANCELED) => Ok(Some(Expiry::ClockChanged)),
            Err(Errno::AGAIN | Errno::INTR) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn set(&self, flags: TimerfdTimerFlags, value: Timespec) -> io::Result<()> {
        let spec = Itimerspec {
            it_interval: NEVER,
            it_value: value,
        };
        timerfd_settime(&self.fd, flags, &spec)?;
        Ok(())
    }
}

impl AsFd for WallAlarm {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl AsRawFd for WallAlarm {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl std::fmt::Debug for WallAlarm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("WallAlarm").finish_non_exhaustive()
    }
}
