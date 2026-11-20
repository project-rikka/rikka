use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_shell::{LayerId, LockSurfaceId, PopupId};
use rikka_compositor_surface::SurfaceId;

use crate::ids::WindowId;
use crate::popup::Parent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Window(WindowId),
    Layer(LayerId),
    Popup(PopupId),
    Lock(LockSurfaceId),
}

impl From<Parent> for Target {
    fn from(parent: Parent) -> Self {
        match parent {
            Parent::Window(window) => Self::Window(window),
            Parent::Layer(layer) => Self::Layer(layer),
            Parent::Popup(popup) => Self::Popup(popup),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Under {
    pub target: Target,
    pub surface: SurfaceId,
    pub local: Point<f64, Logical>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    Surface(Under),
    Frame {
        window: WindowId,
        local: Point<f64, Logical>,
    },
}

impl Hit {
    pub fn target(&self) -> Target {
        match self {
            Self::Surface(under) => under.target,
            Self::Frame { window, .. } => Target::Window(*window),
        }
    }

    pub fn surface(self) -> Option<Under> {
        match self {
            Self::Surface(under) => Some(under),
            Self::Frame { .. } => None,
        }
    }
}
