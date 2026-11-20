use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::ptr::NonNull;

use rikka_gpu::DmabufPlane;
use rikka_gpu::DmabufSpec;
use rikka_wayland_client::{wl_buffer, wl_shm_pool};
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};

use crate::format::PixelFormat;

#[derive(Clone, Copy, Debug)]
pub struct Plane<'a> {
    pub fd: BorrowedFd<'a>,
    pub offset: u32,
    pub stride: u32,
    pub size: usize,
}

pub(crate) enum Memory {
    Shared {
        fd: OwnedFd,
        map: Mapping,
        pool: wl_shm_pool::WlShmPool,
    },
    Device {
        planes: Vec<OwnedPlane>,
        modifier: u64,
        _bo: gbm::BufferObject<()>,
    },
}

pub(crate) struct OwnedPlane {
    pub(crate) fd: OwnedFd,
    pub(crate) offset: u32,
    pub(crate) stride: u32,
}

pub struct CaptureBuffer {
    wl: wl_buffer::WlBuffer,
    memory: Memory,
    format: PixelFormat,
    width: u32,
    height: u32,
    stride: u32,
}

impl CaptureBuffer {
    pub(crate) fn new(
        wl: wl_buffer::WlBuffer,
        memory: Memory,
        format: PixelFormat,
        width: u32,
        height: u32,
        stride: u32,
    ) -> CaptureBuffer {
        CaptureBuffer {
            wl,
            memory,
            format,
            width,
            height,
            stride,
        }
    }

    pub(crate) fn wl(&self) -> &wl_buffer::WlBuffer {
        &self.wl
    }

    #[must_use]
    pub fn format(&self) -> PixelFormat {
        self.format
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn stride(&self) -> u32 {
        self.stride
    }

    #[must_use]
    pub fn is_dmabuf(&self) -> bool {
        matches!(self.memory, Memory::Device { .. })
    }

    #[must_use]
    pub fn modifier(&self) -> Option<u64> {
        match &self.memory {
            Memory::Device { modifier, .. } => Some(*modifier),
            Memory::Shared { .. } => None,
        }
    }

    pub fn with_dmabuf<T>(&self, use_spec: impl FnOnce(&DmabufSpec<'_>) -> T) -> Option<T> {
        let modifier = self.modifier()?;
        let planes: Vec<DmabufPlane<'_>> = self
            .planes()
            .into_iter()
            .map(|plane| DmabufPlane {
                offset: plane.offset,
                stride: plane.stride,
                fd: plane.fd,
            })
            .collect();
        Some(use_spec(&DmabufSpec {
            width: self.width,
            height: self.height,
            fourcc: self.format.fourcc(),
            modifier,
            planes: &planes,
        }))
    }

    #[must_use]
    pub fn planes(&self) -> Vec<Plane<'_>> {
        match &self.memory {
            Memory::Shared { fd, map, .. } => vec![Plane {
                fd: fd.as_fd(),
                offset: 0,
                stride: self.stride,
                size: map.len(),
            }],
            Memory::Device { planes, .. } => planes
                .iter()
                .map(|plane| Plane {
                    fd: plane.fd.as_fd(),
                    offset: plane.offset,
                    stride: plane.stride,
                    size: plane.stride as usize * self.height as usize,
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn pixels(&self) -> Option<&[u8]> {
        match &self.memory {
            Memory::Shared { map, .. } => Some(map.bytes()),
            Memory::Device { .. } => None,
        }
    }
}

impl Drop for CaptureBuffer {
    fn drop(&mut self) {
        self.wl.destroy();
        if let Memory::Shared { pool, .. } = &self.memory {
            pool.destroy();
        }
    }
}

impl std::fmt::Debug for CaptureBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptureBuffer")
            .field("format", &self.format)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("dmabuf", &self.is_dmabuf())
            .finish_non_exhaustive()
    }
}

pub(crate) struct Mapping {
    at: NonNull<u8>,
    len: usize,
}

unsafe impl Send for Mapping {}
unsafe impl Sync for Mapping {}

impl Mapping {
    pub(crate) fn map(fd: BorrowedFd<'_>, len: usize) -> std::io::Result<Mapping> {
        let at = unsafe {
            mmap(
                std::ptr::null_mut(),
                len,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED,
                fd,
                0,
            )
        }?;
        Ok(Mapping {
            at: NonNull::new(at.cast()).expect("mmap should return a non-null address on success"),
            len,
        })
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.at.as_ptr(), self.len) }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        if let Err(error) = unsafe { munmap(self.at.as_ptr().cast(), self.len) } {
            tracing::debug!("cannot unmap a capture buffer: {error}");
        }
    }
}
