use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Once;

use rustix::mm::{self, Advice, MapFlags, ProtFlags};
use zeroize::Zeroize;

use crate::error::{Error, Result};

pub(crate) const KEY_BYTES: usize = 32;

const KEYS: usize = 4;

static UNLOCKABLE: Once = Once::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyRole {
    Master,
    Contents,
    Items,
    Wrap,
}

impl KeyRole {
    fn offset(self) -> usize {
        let index = match self {
            KeyRole::Master => 0,
            KeyRole::Contents => 1,
            KeyRole::Items => 2,
            KeyRole::Wrap => 3,
        };
        index * KEY_BYTES
    }
}

pub(crate) struct KeyPage {
    base: NonNull<u8>,
    len: usize,
}

unsafe impl Send for KeyPage {}

unsafe impl Sync for KeyPage {}

impl KeyPage {
    pub(crate) fn new() -> Result<KeyPage> {
        let len = rustix::param::page_size().max(KEYS * KEY_BYTES);
        let mapped = unsafe {
            mm::mmap_anonymous(
                std::ptr::null_mut(),
                len,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::PRIVATE,
            )
        }
        .map_err(|error| Error::Memory(error.into()))?;
        let base = NonNull::new(mapped.cast::<u8>()).ok_or_else(|| {
            Error::Memory(std::io::Error::other("anonymous mapping returned null"))
        })?;
        let start = base.as_ptr().cast::<c_void>();
        if let Err(error) = unsafe { mm::mlock(start, len) } {
            UNLOCKABLE.call_once(|| {
                tracing::warn!(%error, "failed to lock key memory, keys may reach swap");
            });
        }
        if let Err(error) = unsafe { mm::madvise(start, len, Advice::LinuxDontDump) } {
            tracing::warn!(%error, "failed to keep key memory out of core dumps");
        }
        Ok(KeyPage { base, len })
    }

    pub(crate) fn key(&self, role: KeyRole) -> &[u8; KEY_BYTES] {
        unsafe {
            &*self
                .base
                .as_ptr()
                .add(role.offset())
                .cast::<[u8; KEY_BYTES]>()
        }
    }

    pub(crate) fn key_mut(&mut self, role: KeyRole) -> &mut [u8; KEY_BYTES] {
        unsafe {
            &mut *self
                .base
                .as_ptr()
                .add(role.offset())
                .cast::<[u8; KEY_BYTES]>()
        }
    }
}

impl Drop for KeyPage {
    fn drop(&mut self) {
        unsafe { std::slice::from_raw_parts_mut(self.base.as_ptr(), self.len) }.zeroize();
        if let Err(error) = unsafe { mm::munmap(self.base.as_ptr().cast(), self.len) } {
            tracing::warn!(%error, "failed to unmap key memory");
        }
    }
}

impl std::fmt::Debug for KeyPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("KeyPage").finish_non_exhaustive()
    }
}
