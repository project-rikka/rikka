use std::fmt;

use drm::control::{connector, crtc, framebuffer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrtcId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectorId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FramebufferId(u32);

impl CrtcId {
    pub(crate) fn from_handle(handle: crtc::Handle) -> Self {
        Self(handle.into())
    }

    pub(crate) fn handle(self) -> crtc::Handle {
        drm::control::from_u32(self.0).expect("a CRTC id is never zero")
    }

    pub(crate) fn raw(self) -> u32 {
        self.0
    }
}

impl ConnectorId {
    pub(crate) fn from_handle(handle: connector::Handle) -> Self {
        Self(handle.into())
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl FramebufferId {
    pub(crate) fn from_handle(handle: framebuffer::Handle) -> Self {
        Self(handle.into())
    }

    pub(crate) fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Display for CrtcId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "CRTC {}", self.0)
    }
}

impl fmt::Display for ConnectorId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "connector {}", self.0)
    }
}

impl fmt::Display for FramebufferId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "framebuffer {}", self.0)
    }
}
