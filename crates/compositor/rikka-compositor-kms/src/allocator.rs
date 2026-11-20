use std::io;

use drm::buffer::PlanarBuffer as _;
use gbm::BufferObjectFlags;
use rikka_compositor_types::{Format, Fourcc, Modifier};

use crate::dmabuf::{Dmabuf, DmabufPlane};
use crate::drm_fd::DrmFd;
use crate::dumb::{Dumb, Mapped};
use crate::error::{Error, Result};
use crate::format::{drm_fourcc, drm_modifier};
use crate::framebuffer::{Backing, Framebuffer, Layout, Origin};
use crate::negotiate::Candidate;

pub(crate) struct Allocator {
    gbm: Option<gbm::Device<DrmFd>>,
    fd: DrmFd,
}

pub(crate) enum Cursor {
    Gbm(gbm::BufferObject<()>),
    Mapped(Mapped),
}

impl Allocator {
    pub(crate) fn new(fd: &DrmFd) -> Self {
        let gbm = gbm::Device::new(fd.clone())
            .inspect_err(|error| {
                tracing::info!("GBM unavailable, falling back to dumb buffers: {error}")
            })
            .ok();
        Self {
            gbm,
            fd: fd.clone(),
        }
    }

    pub(crate) fn scanout(
        &self,
        width: u32,
        height: u32,
        candidate: &Candidate,
    ) -> Result<(Framebuffer, Dmabuf)> {
        let Some(gbm) = self.gbm.as_ref() else {
            return match candidate.linear {
                true => self.shared_dumb(width, height, candidate),
                false => Err(Error::Gbm(io::Error::from(io::ErrorKind::Unsupported))),
            };
        };
        let fourcc =
            drm_fourcc(candidate.fourcc).ok_or(Error::UnknownFourcc(candidate.fourcc.0))?;
        let mut usage = BufferObjectFlags::SCANOUT | BufferObjectFlags::RENDERING;
        if candidate.linear {
            usage |= BufferObjectFlags::LINEAR;
        }
        let buffer: gbm::BufferObject<()> = match candidate.is_implicit() {
            true => gbm
                .create_buffer_object(width, height, fourcc, usage)
                .map_err(Error::Allocation)?,
            false => gbm
                .create_buffer_object_with_modifiers2(
                    width,
                    height,
                    fourcc,
                    candidate
                        .modifiers
                        .iter()
                        .map(|modifier| drm_modifier(*modifier)),
                    usage,
                )
                .map_err(Error::Allocation)?,
        };
        let actual = match candidate.linear {
            true => Modifier::LINEAR,
            false => Modifier(u64::from(buffer.modifier())),
        };
        let dmabuf = export(
            &buffer,
            width,
            height,
            Format::new(candidate.fourcc, actual),
        )?;
        let scanned = match candidate.is_implicit() {
            true => Modifier::INVALID,
            false => actual,
        };
        let layout = layout_of(
            &buffer,
            width,
            height,
            Format::new(candidate.fourcc, scanned),
        );
        let framebuffer =
            Framebuffer::add(&self.fd, layout, Origin::Swapchain, Backing::owning(buffer))?;
        Ok((framebuffer, dmabuf))
    }

    pub(crate) fn cursor(&self, width: u32, height: u32) -> Result<(Cursor, Framebuffer)> {
        let Some(gbm) = &self.gbm else {
            let (framebuffer, mapped) =
                self.mapped(width, height, Fourcc::ARGB8888, Origin::Cursor)?;
            return Ok((Cursor::Mapped(mapped), framebuffer));
        };
        let buffer: gbm::BufferObject<()> = gbm
            .create_buffer_object(
                width,
                height,
                gbm::Format::Argb8888,
                BufferObjectFlags::CURSOR | BufferObjectFlags::WRITE,
            )
            .map_err(Error::Allocation)?;
        let layout = layout_of(
            &buffer,
            width,
            height,
            Format::new(Fourcc::ARGB8888, Modifier::INVALID),
        );
        let framebuffer = Framebuffer::add(&self.fd, layout, Origin::Cursor, Backing::BORROWED)?;
        Ok((Cursor::Gbm(buffer), framebuffer))
    }

    pub(crate) fn mapped(
        &self,
        width: u32,
        height: u32,
        fourcc: Fourcc,
        origin: Origin,
    ) -> Result<(Framebuffer, Mapped)> {
        let dumb = Dumb::create(&self.fd, width, height)?;
        let mapped = dumb.map()?;
        let layout = dumb_layout(&dumb, width, height, Format::new(fourcc, Modifier::INVALID));
        let framebuffer = Framebuffer::add(&self.fd, layout, origin, Backing::owning(dumb))?;
        Ok((framebuffer, mapped))
    }

    fn shared_dumb(
        &self,
        width: u32,
        height: u32,
        candidate: &Candidate,
    ) -> Result<(Framebuffer, Dmabuf)> {
        let dumb = Dumb::create(&self.fd, width, height)?;
        let dmabuf = Dmabuf::new(
            width,
            height,
            Format::new(candidate.fourcc, Modifier::LINEAR),
            vec![DmabufPlane {
                fd: dumb.export()?,
                offset: 0,
                stride: dumb.stride(),
            }],
        );
        let scanned = match candidate.is_implicit() {
            true => Modifier::INVALID,
            false => Modifier::LINEAR,
        };
        let layout = dumb_layout(&dumb, width, height, Format::new(candidate.fourcc, scanned));
        let framebuffer =
            Framebuffer::add(&self.fd, layout, Origin::Swapchain, Backing::owning(dumb))?;
        Ok((framebuffer, dmabuf))
    }
}

fn dumb_layout(dumb: &Dumb, width: u32, height: u32, format: Format) -> Layout {
    Layout {
        width,
        height,
        format,
        handles: [Some(dumb.handle()), None, None, None],
        pitches: [dumb.stride(), 0, 0, 0],
        offsets: [0; 4],
    }
}

impl Cursor {
    pub(crate) fn stride(&self) -> u32 {
        match self {
            Self::Gbm(buffer) => buffer.stride(),
            Self::Mapped(mapped) => mapped.stride(),
        }
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<()> {
        match self {
            Self::Gbm(buffer) => buffer.write(bytes).map_err(Error::Allocation),
            Self::Mapped(mapped) => {
                let into = mapped.bytes_mut();
                let length = bytes.len().min(into.len());
                into[..length].copy_from_slice(&bytes[..length]);
                Ok(())
            }
        }
    }
}

fn export(
    buffer: &gbm::BufferObject<()>,
    width: u32,
    height: u32,
    format: Format,
) -> Result<Dmabuf> {
    let planes = (0..plane_count(buffer))
        .map(|plane| {
            let fd = buffer
                .fd_for_plane(plane)
                .map_err(|_| Error::Export(io::Error::other("GBM would not export the plane")))?;
            Ok(DmabufPlane {
                fd,
                offset: buffer.offset(plane),
                stride: buffer.stride_for_plane(plane),
            })
        })
        .collect::<Result<_>>()?;
    Ok(Dmabuf::new(width, height, format, planes))
}

fn layout_of(buffer: &gbm::BufferObject<()>, width: u32, height: u32, format: Format) -> Layout {
    let mut offsets = [0; 4];
    for plane in 0..plane_count(buffer) {
        offsets[plane as usize] = buffer.offset(plane);
    }
    Layout {
        width,
        height,
        format,
        handles: buffer.handles(),
        pitches: buffer.pitches(),
        offsets,
    }
}

fn plane_count(buffer: &gbm::BufferObject<()>) -> i32 {
    i32::try_from(buffer.plane_count()).unwrap_or(1).clamp(1, 4)
}
