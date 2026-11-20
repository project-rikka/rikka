use std::io;
use std::time::{Duration, Instant};

use rikka_types::CancelToken;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;

use super::cancel_signal::CancelSignal;
use super::hotplug::Hotplug;

const SUBSYSTEM: &str = "hidraw";

pub(crate) struct HidrawMonitor {
    socket: udev::MonitorSocket,
}

impl HidrawMonitor {
    pub(crate) fn new() -> io::Result<HidrawMonitor> {
        let socket = udev::MonitorBuilder::new()?
            .match_subsystem(SUBSYSTEM)?
            .listen()?;
        Ok(HidrawMonitor { socket })
    }
}

impl Hotplug for HidrawMonitor {
    fn wait(&mut self, cancel: &CancelToken, timeout: Duration) -> io::Result<()> {
        let cancelled = CancelSignal::new(cancel)?;
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let timeout = Timespec::try_from(left).map_err(|_| io::ErrorKind::InvalidInput)?;
            let mut fds = [
                PollFd::new(&self.socket, PollFlags::IN),
                PollFd::new(&cancelled, PollFlags::IN),
            ];
            match poll(&mut fds, Some(&timeout)) {
                Err(Errno::INTR) => {}
                Err(error) => return Err(error.into()),
                Ok(_) => break,
            }
        }
        self.socket.iter().for_each(drop);
        Ok(())
    }
}

impl std::fmt::Debug for HidrawMonitor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HidrawMonitor")
            .finish_non_exhaustive()
    }
}
