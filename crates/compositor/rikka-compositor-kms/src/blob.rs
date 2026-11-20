use std::io;
use std::os::fd::AsFd;

use drm::control::{Device as _, property};
use rustix::ioctl::{Updater, ioctl, opcode};

use crate::damage::{Damage, MAX_RECTS};
use crate::drm_fd::DrmFd;
use crate::hdr::HdrMetadata;
use crate::mode::Mode;

#[derive(Debug)]
pub(crate) struct Blob {
    fd: DrmFd,
    id: u64,
}

#[repr(C)]
struct CreateBlob {
    data: u64,
    length: u32,
    blob_id: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DamageRect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

const CREATE_BLOB: rustix::ioctl::Opcode = opcode::read_write::<CreateBlob>(b'd', 0xBD);

impl Blob {
    pub(crate) fn mode(fd: &DrmFd, mode: &Mode) -> io::Result<Self> {
        let property::Value::Blob(id) = fd.create_property_blob(mode.raw())? else {
            return Err(io::Error::other("kernel returned a non-blob property"));
        };
        Ok(Self { fd: fd.clone(), id })
    }

    pub(crate) fn damage(fd: &DrmFd, damage: &Damage) -> io::Result<Self> {
        let mut rects = [DamageRect::default(); MAX_RECTS];
        let mut count = 0;
        for rect in damage.rects() {
            rects[count] = DamageRect {
                x1: rect.x,
                y1: rect.y,
                x2: rect.right(),
                y2: rect.bottom(),
            };
            count += 1;
        }
        let description = &rects[..count];
        Self::create(
            fd,
            description.as_ptr() as u64,
            std::mem::size_of_val(description),
        )
    }

    pub(crate) fn hdr_metadata(fd: &DrmFd, metadata: &HdrMetadata) -> io::Result<Self> {
        let bytes = metadata.blob();
        Self::create(fd, bytes.as_ptr() as u64, bytes.len())
    }

    fn create(fd: &DrmFd, data: u64, length: usize) -> io::Result<Self> {
        let mut request = CreateBlob {
            data,
            length: length as u32,
            blob_id: 0,
        };
        let updater = unsafe { Updater::<CREATE_BLOB, CreateBlob>::new(&mut request) };
        unsafe { ioctl(fd.as_fd(), updater) }?;
        Ok(Self {
            fd: fd.clone(),
            id: u64::from(request.blob_id),
        })
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }
}

impl Drop for Blob {
    fn drop(&mut self) {
        if let Err(error) = self.fd.destroy_property_blob(self.id) {
            tracing::debug!("failed to destroy property blob: {error}");
        }
    }
}
