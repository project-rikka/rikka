use std::fmt;
use std::os::fd::{AsFd, OwnedFd};
use std::ptr::NonNull;

use drm::buffer::Handle as BufferHandle;
use drm::control::Device as _;
use rustix::mm::{MapFlags, ProtFlags};

use crate::drm_fd::DrmFd;
use crate::error::{Error, Result};

const BITS_PER_PIXEL: u32 = 32;

pub(crate) struct Dumb {
    fd: DrmFd,
    handle: BufferHandle,
    stride: u32,
    length: usize,
    width: u32,
    height: u32,
}

pub struct Mapped {
    pixels: NonNull<u8>,
    length: usize,
    stride: u32,
    width: u32,
    height: u32,
}

unsafe impl Send for Mapped {}

impl Dumb {
    pub(crate) fn create(fd: &DrmFd, width: u32, height: u32) -> Result<Self> {
        let created =
            drm_ffi::mode::dumbbuffer::create(fd.as_fd(), width, height, BITS_PER_PIXEL, 0)
                .map_err(Error::Allocation)?;
        let handle =
            BufferHandle::from(std::num::NonZeroU32::new(created.handle).ok_or_else(|| {
                Error::Allocation(std::io::Error::other("no dumb buffer handle"))
            })?);
        let mut dumb = Self {
            fd: fd.clone(),
            handle,
            stride: created.pitch,
            length: 0,
            width,
            height,
        };
        dumb.length = usize::try_from(created.size).map_err(|_| {
            Error::Allocation(std::io::Error::other("dumb buffer too large to map"))
        })?;
        Ok(dumb)
    }

    pub(crate) fn map(&self) -> Result<Mapped> {
        let offset = drm_ffi::mode::dumbbuffer::map(self.fd.as_fd(), self.handle.into(), 0, 0)
            .map_err(Error::Allocation)?
            .offset;
        let pixels = unsafe {
            rustix::mm::mmap(
                std::ptr::null_mut(),
                self.length,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED,
                self.fd.as_fd(),
                offset,
            )
        }
        .map_err(|errno| Error::Allocation(errno.into()))?;
        let pixels = NonNull::new(pixels.cast::<u8>()).ok_or_else(|| {
            Error::Allocation(std::io::Error::other("dumb buffer mapped at address 0"))
        })?;
        Ok(Mapped {
            pixels,
            length: self.length,
            stride: self.stride,
            width: self.width,
            height: self.height,
        })
    }

    pub(crate) fn export(&self) -> Result<OwnedFd> {
        self.fd
            .buffer_to_prime_fd(self.handle, drm::CLOEXEC | drm::RDWR)
            .map_err(Error::Export)
    }

    pub(crate) fn handle(&self) -> BufferHandle {
        self.handle
    }

    pub(crate) fn stride(&self) -> u32 {
        self.stride
    }
}

impl Drop for Dumb {
    fn drop(&mut self) {
        if let Err(error) = drm_ffi::mode::dumbbuffer::destroy(self.fd.as_fd(), self.handle.into())
        {
            tracing::debug!("failed to destroy dumb buffer: {error}");
        }
    }
}

impl Mapped {
    pub fn stride(&self) -> u32 {
        self.stride
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.pixels.as_ptr(), self.length) }
    }

    pub fn bytes_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.pixels.as_ptr(), self.length) }
    }
}

impl Drop for Mapped {
    fn drop(&mut self) {
        if let Err(error) = unsafe { rustix::mm::munmap(self.pixels.as_ptr().cast(), self.length) }
        {
            tracing::debug!("failed to unmap dumb buffer: {error}");
        }
    }
}

impl fmt::Debug for Mapped {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Mapped")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .finish()
    }
}

impl fmt::Debug for Dumb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Dumb")
            .field("handle", &u32::from(self.handle))
            .finish()
    }
}
