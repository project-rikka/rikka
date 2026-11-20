use std::fmt;
use std::io;
use std::os::fd::BorrowedFd;
use std::ptr::{self, NonNull};

use rustix::mm::{MapFlags, MremapFlags, ProtFlags, mmap, mremap, munmap};

pub(crate) struct Mapping {
    base: NonNull<u8>,
    len: usize,
    writable: bool,
}

impl Mapping {
    pub(crate) fn new(fd: BorrowedFd<'_>, len: usize) -> io::Result<Self> {
        match map(fd, len, ProtFlags::READ | ProtFlags::WRITE) {
            Ok(base) => Ok(Self {
                base,
                len,
                writable: true,
            }),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => Ok(Self {
                base: map(fd, len, ProtFlags::READ)?,
                len,
                writable: false,
            }),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn grow(&mut self, len: usize) -> io::Result<()> {
        let base = unsafe {
            mremap(
                self.base.as_ptr().cast(),
                self.len,
                len,
                MremapFlags::MAYMOVE,
            )
        }?;
        self.base = NonNull::new(base.cast::<u8>()).ok_or(io::ErrorKind::InvalidData)?;
        self.len = len;
        Ok(())
    }

    pub(crate) fn base(&self) -> *const u8 {
        self.base.as_ptr()
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn writable(&self) -> bool {
        self.writable
    }
}

fn map(fd: BorrowedFd<'_>, len: usize, protection: ProtFlags) -> io::Result<NonNull<u8>> {
    let base = unsafe { mmap(ptr::null_mut(), len, protection, MapFlags::SHARED, fd, 0) }?;
    NonNull::new(base.cast::<u8>()).ok_or_else(|| io::ErrorKind::InvalidData.into())
}

impl Drop for Mapping {
    fn drop(&mut self) {
        if let Err(error) = unsafe { munmap(self.base.as_ptr().cast(), self.len) } {
            tracing::warn!("failed to unmap shm pool: {error}");
        }
    }
}

impl fmt::Debug for Mapping {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Mapping")
            .field("base", &self.base)
            .field("len", &self.len)
            .finish()
    }
}
