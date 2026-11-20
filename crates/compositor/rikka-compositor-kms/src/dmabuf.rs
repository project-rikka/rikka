use std::io;
use std::os::fd::OwnedFd;

use rikka_compositor_types::Format;

#[derive(Debug)]
pub struct Dmabuf {
    width: u32,
    height: u32,
    format: Format,
    planes: Vec<DmabufPlane>,
}

#[derive(Debug)]
pub struct DmabufPlane {
    pub fd: OwnedFd,
    pub offset: u32,
    pub stride: u32,
}

impl Dmabuf {
    pub fn new(width: u32, height: u32, format: Format, planes: Vec<DmabufPlane>) -> Self {
        Self {
            width,
            height,
            format,
            planes,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> Format {
        self.format
    }

    pub fn planes(&self) -> &[DmabufPlane] {
        &self.planes
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        let planes = self
            .planes
            .iter()
            .map(|plane| {
                Ok(DmabufPlane {
                    fd: plane.fd.try_clone()?,
                    offset: plane.offset,
                    stride: plane.stride,
                })
            })
            .collect::<io::Result<_>>()?;
        Ok(Self { planes, ..*self })
    }
}
