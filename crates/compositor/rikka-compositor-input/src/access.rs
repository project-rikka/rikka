use std::fmt;
use std::io;
use std::os::fd::OwnedFd;
use std::path::Path;

use input::LibinputInterface;
use rustix::fs::OFlags;

type Open = Box<dyn FnMut(&Path, OFlags) -> io::Result<OwnedFd> + Send>;
type Close = Box<dyn FnMut(OwnedFd) + Send>;

pub struct DeviceAccess {
    open: Open,
    close: Close,
}

impl DeviceAccess {
    pub fn new(
        open: impl FnMut(&Path, OFlags) -> io::Result<OwnedFd> + Send + 'static,
        close: impl FnMut(OwnedFd) + Send + 'static,
    ) -> Self {
        Self {
            open: Box::new(open),
            close: Box::new(close),
        }
    }
}

impl LibinputInterface for DeviceAccess {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        (self.open)(path, OFlags::from_bits_retain(flags as u32)).map_err(|error| {
            tracing::debug!("failed to open input device {}: {error}", path.display());
            error.raw_os_error().unwrap_or(libc::EIO)
        })
    }

    fn close_restricted(&mut self, fd: OwnedFd) {
        (self.close)(fd);
    }
}

impl fmt::Debug for DeviceAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeviceAccess")
            .finish_non_exhaustive()
    }
}
