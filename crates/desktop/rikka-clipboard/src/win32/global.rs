use std::io;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};

use rikka_clipboard_core::{Error, Result};

pub(super) struct Global(HGLOBAL);

impl Global {
    pub(super) fn holding(bytes: &[u8]) -> Result<Global> {
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) }.map_err(io_error)?;
        let global = Global(handle);
        let locked = Locked::new(handle)?;
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), locked.address, bytes.len());
        }
        drop(locked);
        Ok(global)
    }

    pub(super) fn into_handle(self) -> HANDLE {
        let handle = HANDLE(self.0.0);
        std::mem::forget(self);
        handle
    }

    pub(super) fn from_handle(handle: HANDLE) -> Global {
        Global(HGLOBAL(handle.0))
    }
}

impl Drop for Global {
    fn drop(&mut self) {
        if let Err(error) = unsafe { GlobalFree(Some(self.0)) } {
            tracing::debug!("failed to free clipboard memory: {error}");
        }
    }
}

struct Locked {
    handle: HGLOBAL,
    address: *mut u8,
}

impl Locked {
    fn new(handle: HGLOBAL) -> Result<Locked> {
        let address = unsafe { GlobalLock(handle) }.cast::<u8>();
        if address.is_null() {
            return Err(io_error(windows::core::Error::from_thread()));
        }
        Ok(Locked { handle, address })
    }
}

impl Drop for Locked {
    fn drop(&mut self) {
        if let Err(error) = unsafe { GlobalUnlock(self.handle) }
            && error.code().is_err()
        {
            tracing::debug!("failed to unlock clipboard memory: {error}");
        }
    }
}

pub(super) fn read(handle: HANDLE, limit: usize) -> Result<Vec<u8>> {
    let handle = HGLOBAL(handle.0);
    let size = unsafe { GlobalSize(handle) };
    if size > limit {
        return Err(Error::Io(io::Error::other(format!(
            "clipboard data of {size} bytes exceeds {limit}"
        ))));
    }
    let locked = Locked::new(handle)?;
    let bytes = unsafe { std::slice::from_raw_parts(locked.address, size) }.to_vec();
    Ok(bytes)
}

pub(super) fn io_error(error: windows::core::Error) -> Error {
    Error::Io(io::Error::other(error))
}
