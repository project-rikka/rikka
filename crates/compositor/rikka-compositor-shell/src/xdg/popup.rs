use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_protocols::xdg_popup::XdgPopup;
use rikka_compositor_protocols::xdg_surface::XdgSurface;
use rikka_compositor_protocols::xdg_wm_base::XdgWmBase;
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;

use super::positioner::Positioner;
use super::window::WindowState;
use crate::ids::{LayerId, PopupId, ToplevelId};
use crate::lifecycle::Lifecycle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PopupParent {
    Toplevel(ToplevelId),
    Popup(PopupId),
    Layer(LayerId),
}

#[derive(Debug)]
pub struct Popup {
    pub(crate) resource: XdgPopup,
    pub(crate) xdg_surface: XdgSurface,
    pub(crate) wm_base: XdgWmBase,
    pub(crate) surface: SurfaceId,
    pub(crate) lifecycle: Lifecycle<Rect<i32, Logical>, WindowState>,
    pub(crate) parent: Option<PopupParent>,
    pub(crate) positioner: Positioner,
    pub(crate) reposition: Option<u32>,
    pub(crate) dismissed: bool,
}

impl Popup {
    pub fn surface(&self) -> SurfaceId {
        self.surface
    }

    pub fn client(&self) -> ClientId {
        self.resource.client()
    }

    pub fn version(&self) -> u32 {
        self.resource.version()
    }

    pub fn parent(&self) -> Option<PopupParent> {
        self.parent
    }

    pub fn positioner(&self) -> &Positioner {
        &self.positioner
    }

    pub fn geometry(&self) -> Option<Rect<i32, Logical>> {
        self.lifecycle.current().copied()
    }

    pub fn window_geometry(&self) -> Option<Rect<i32, Logical>> {
        self.lifecycle.state.geometry
    }

    pub fn awaiting_ack(&self) -> bool {
        self.lifecycle.awaiting_ack()
    }

    pub fn initial_configure_sent(&self) -> bool {
        self.lifecycle.configure_sent()
    }

    pub fn mapped(&self) -> bool {
        self.lifecycle.mapped()
    }

    pub fn dismissed(&self) -> bool {
        self.dismissed
    }
}
