use std::fs::File;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::ptr::NonNull;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, PAGE_READONLY,
    UnmapViewOfFile,
};
use windows::core::PCWSTR;

pub(super) fn map(file: &File, len: usize) -> io::Result<NonNull<u8>> {
    let section = unsafe {
        CreateFileMappingW(
            HANDLE(file.as_raw_handle()),
            None,
            PAGE_READONLY,
            0,
            0,
            PCWSTR::null(),
        )
    }?;
    let view = unsafe { MapViewOfFile(section, FILE_MAP_READ, 0, 0, len) };
    let mapped = NonNull::new(view.Value.cast::<u8>()).ok_or_else(io::Error::last_os_error);
    let closed = unsafe { CloseHandle(section) };
    match (mapped, closed) {
        (Ok(start), Ok(())) => Ok(start),
        (Ok(start), Err(error)) => {
            unsafe { unmap(start, len) };
            Err(error.into())
        }
        (Err(error), _) => Err(error),
    }
}

pub(super) unsafe fn unmap(start: NonNull<u8>, _: usize) {
    let view = MEMORY_MAPPED_VIEW_ADDRESS {
        Value: start.as_ptr().cast(),
    };
    let _ = unsafe { UnmapViewOfFile(view) };
}
