use std::io;
use std::os::fd::AsFd;

use drm::buffer::Handle as BufferHandle;
use drm::control::Device as _;
use rikka_compositor_types::{Format, Modifier};

use crate::dmabuf::Dmabuf;
use crate::drm_fd::DrmFd;
use crate::error::{Error, Result};
use crate::framebuffer::{Backing, Framebuffer, Layout, Origin};

const MAX_PLANES: usize = 4;

pub(crate) fn import(
    fd: &DrmFd,
    explicit_modifiers: bool,
    dmabuf: &Dmabuf,
    origin: Origin,
) -> Result<Framebuffer> {
    let planes = dmabuf.planes();
    if planes.is_empty() || planes.len() > MAX_PLANES {
        return Err(Error::Import(io::Error::from(io::ErrorKind::InvalidInput)));
    }
    let format = match dmabuf.format() {
        format if explicit_modifiers || format.modifier.is_implicit() => format,
        format if format.modifier == Modifier::LINEAR => {
            Format::new(format.fourcc, Modifier::INVALID)
        }
        _ => return Err(Error::Import(io::Error::from(io::ErrorKind::Unsupported))),
    };
    let mut handles: [Option<BufferHandle>; MAX_PLANES] = [None; MAX_PLANES];
    let mut pitches = [0; MAX_PLANES];
    let mut offsets = [0; MAX_PLANES];
    let mut gem_handles: Vec<BufferHandle> = Vec::new();
    let mut imported = Ok(());
    for (index, plane) in planes.iter().enumerate() {
        match fd.prime_fd_to_buffer(plane.fd.as_fd()) {
            Ok(handle) => {
                if !gem_handles.contains(&handle) {
                    gem_handles.push(handle);
                }
                handles[index] = Some(handle);
                pitches[index] = plane.stride;
                offsets[index] = plane.offset;
            }
            Err(error) => {
                imported = Err(Error::Import(error));
                break;
            }
        }
    }
    let framebuffer = imported.and_then(|()| {
        Framebuffer::add(
            fd,
            Layout {
                width: dmabuf.width(),
                height: dmabuf.height(),
                format,
                handles,
                pitches,
                offsets,
            },
            origin,
            Backing::BORROWED,
        )
    });
    for handle in gem_handles {
        if let Err(error) = fd.close_buffer(handle) {
            tracing::debug!("failed to close GEM handle: {error}");
        }
    }
    framebuffer
}
