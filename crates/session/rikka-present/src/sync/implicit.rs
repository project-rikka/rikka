use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

use super::timelines::{ioctl_readwrite, ioctl_write};

const SYNC_READ: u32 = 1 << 0;
const SYNC_WRITE: u32 = 1 << 1;
const EXPORT_SYNC_FILE: libc::c_ulong = ioctl_readwrite(b'b', 2, size_of::<SyncFile>());
const IMPORT_SYNC_FILE: libc::c_ulong = ioctl_write(b'b', 3, size_of::<SyncFile>());

#[repr(C)]
struct SyncFile {
    flags: u32,
    fd: i32,
}

pub(crate) fn written_by(dmabuf: BorrowedFd<'_>, fence: BorrowedFd<'_>) -> io::Result<()> {
    let mut args = SyncFile {
        flags: SYNC_WRITE,
        fd: fence.as_raw_fd(),
    };
    let done = unsafe { libc::ioctl(dmabuf.as_raw_fd(), IMPORT_SYNC_FILE, &raw mut args) };
    match done {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}

pub(crate) fn readers_of(dmabuf: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    let mut args = SyncFile {
        flags: SYNC_READ | SYNC_WRITE,
        fd: -1,
    };
    let done = unsafe { libc::ioctl(dmabuf.as_raw_fd(), EXPORT_SYNC_FILE, &raw mut args) };
    match done {
        0 => Ok(unsafe { OwnedFd::from_raw_fd(args.fd) }),
        _ => Err(io::Error::last_os_error()),
    }
}

pub(crate) fn wait_signalled(fence: BorrowedFd<'_>) -> io::Result<()> {
    rikka_worker::wait_readable(fence, None).map(drop)
}
