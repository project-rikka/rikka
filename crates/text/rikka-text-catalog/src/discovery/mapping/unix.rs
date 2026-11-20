use std::fs::File;
use std::io;
use std::ptr::NonNull;

use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};

pub(super) fn map(file: &File, len: usize) -> io::Result<NonNull<u8>> {
    let start = unsafe {
        mmap(
            std::ptr::null_mut(),
            len,
            ProtFlags::READ,
            MapFlags::PRIVATE,
            file,
            0,
        )
    }?;
    NonNull::new(start.cast::<u8>()).ok_or_else(|| io::Error::other("mmap returned a null mapping"))
}

pub(super) unsafe fn unmap(start: NonNull<u8>, len: usize) {
    let _ = unsafe { munmap(start.as_ptr().cast(), len) };
}
