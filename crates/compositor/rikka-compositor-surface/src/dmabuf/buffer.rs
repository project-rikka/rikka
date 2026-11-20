use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rikka_compositor_geometry::{Buffer, Size};
pub use rikka_compositor_protocols::zwp_linux_buffer_params_v1::Flags as DmabufFlags;
use rikka_compositor_types::Format;
use rustix::fs::Dev;

#[derive(Debug)]
pub struct DmabufPlane {
    pub(crate) fd: OwnedFd,
    pub(crate) offset: u32,
    pub(crate) stride: u32,
}

impl DmabufPlane {
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn stride(&self) -> u32 {
        self.stride
    }
}

#[derive(Debug)]
pub struct Dmabuf {
    pub(crate) size: Size<i32, Buffer>,
    pub(crate) format: Format,
    pub(crate) flags: DmabufFlags,
    pub(crate) planes: Vec<DmabufPlane>,
    pub(crate) sampling_device: Option<Dev>,
    pub(crate) fenced_planes: u8,
}

impl Dmabuf {
    pub fn size(&self) -> Size<i32, Buffer> {
        self.size
    }

    pub fn format(&self) -> Format {
        self.format
    }

    pub fn flags(&self) -> DmabufFlags {
        self.flags
    }

    pub fn planes(&self) -> &[DmabufPlane] {
        &self.planes
    }

    pub fn sampling_device(&self) -> Option<Dev> {
        self.sampling_device
    }
}

pub(crate) fn device_bytes(device: Dev) -> [u8; 8] {
    device.to_ne_bytes()
}
