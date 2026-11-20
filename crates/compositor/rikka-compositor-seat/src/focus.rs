use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Focus {
    pub surface: SurfaceId,
    pub origin: Point<f64, Logical>,
}

impl Focus {
    pub fn new(surface: SurfaceId, origin: Point<f64, Logical>) -> Self {
        Self { surface, origin }
    }

    pub fn local(&self, location: Point<f64, Logical>) -> Point<f64, Logical> {
        location - self.origin
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Focused {
    pub(crate) focus: Focus,
    pub(crate) surface: WlSurface,
}

impl Focused {
    pub(crate) fn client(&self) -> ClientId {
        self.surface.client()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyboardFocus {
    Surface(SurfaceId),
    X11 { window: u32, surface: SurfaceId },
}

impl KeyboardFocus {
    pub fn surface(&self) -> SurfaceId {
        match *self {
            Self::Surface(surface) | Self::X11 { surface, .. } => surface,
        }
    }
}

impl From<SurfaceId> for KeyboardFocus {
    fn from(surface: SurfaceId) -> Self {
        Self::Surface(surface)
    }
}
