use std::fmt;
use std::sync::Arc;

use drm::buffer::{Handle as BufferHandle, PlanarBuffer};
use drm::control::{Device as _, FbCmd2Flags, framebuffer};
use drm_fourcc::{DrmFourcc, DrmModifier};
use rikka_compositor_types::{Format, Modifier};

use crate::drm_fd::DrmFd;
use crate::error::{Error, Result};
use crate::format::{drm_fourcc, drm_modifier};
use crate::ids::FramebufferId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Swapchain,
    Client,
    Cursor,
}

#[derive(Clone)]
pub struct Framebuffer(Arc<Inner>);

struct Inner {
    handle: framebuffer::Handle,
    fd: DrmFd,
    width: u32,
    height: u32,
    format: Format,
    origin: Origin,
    _backing: Backing,
}

pub(crate) struct Backing {
    _kept: Option<Box<dyn Send + Sync>>,
}

impl Backing {
    pub(crate) const BORROWED: Self = Self { _kept: None };

    pub(crate) fn owning(memory: impl Send + Sync + 'static) -> Self {
        Self {
            _kept: Some(Box::new(memory)),
        }
    }
}

pub(crate) struct Layout {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: Format,
    pub(crate) handles: [Option<BufferHandle>; 4],
    pub(crate) pitches: [u32; 4],
    pub(crate) offsets: [u32; 4],
}

struct PlanarDescription {
    layout: Layout,
    fourcc: DrmFourcc,
}

impl Framebuffer {
    pub(crate) fn add(
        fd: &DrmFd,
        layout: Layout,
        origin: Origin,
        backing: Backing,
    ) -> Result<Self> {
        let fourcc =
            drm_fourcc(layout.format.fourcc).ok_or(Error::UnknownFourcc(layout.format.fourcc.0))?;
        let flags = match layout.format.modifier.is_implicit() {
            true => FbCmd2Flags::empty(),
            false => FbCmd2Flags::MODIFIERS,
        };
        let (width, height, format) = (layout.width, layout.height, layout.format);
        let description = PlanarDescription { layout, fourcc };
        let handle = fd
            .add_planar_framebuffer(&description, flags)
            .map_err(Error::Framebuffer)?;
        Ok(Self(Arc::new(Inner {
            handle,
            fd: fd.clone(),
            width,
            height,
            format,
            origin,
            _backing: backing,
        })))
    }

    pub fn id(&self) -> FramebufferId {
        FramebufferId::from_handle(self.0.handle)
    }

    pub fn width(&self) -> u32 {
        self.0.width
    }

    pub fn height(&self) -> u32 {
        self.0.height
    }

    pub fn format(&self) -> Format {
        self.0.format
    }

    pub fn origin(&self) -> Origin {
        self.0.origin
    }

    pub(crate) fn is_held_elsewhere(&self) -> bool {
        Arc::strong_count(&self.0) > 1
    }
}

impl PlanarBuffer for PlanarDescription {
    fn size(&self) -> (u32, u32) {
        (self.layout.width, self.layout.height)
    }

    fn format(&self) -> DrmFourcc {
        self.fourcc
    }

    fn modifier(&self) -> Option<DrmModifier> {
        let modifier: Modifier = self.layout.format.modifier;
        (!modifier.is_implicit()).then(|| drm_modifier(modifier))
    }

    fn pitches(&self) -> [u32; 4] {
        self.layout.pitches
    }

    fn handles(&self) -> [Option<BufferHandle>; 4] {
        self.layout.handles
    }

    fn offsets(&self) -> [u32; 4] {
        self.layout.offsets
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Err(error) = self.fd.destroy_framebuffer(self.handle) {
            tracing::debug!(
                "failed to remove framebuffer {}: {error}",
                u32::from(self.handle)
            );
        }
    }
}

impl fmt::Debug for Framebuffer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Framebuffer")
            .field("id", &self.id())
            .field("width", &self.0.width)
            .field("height", &self.0.height)
            .field("format", &self.0.format)
            .field("origin", &self.0.origin)
            .finish()
    }
}
