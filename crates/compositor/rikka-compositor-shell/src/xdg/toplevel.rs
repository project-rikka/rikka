use rikka_compositor_geometry::{Logical, Rect, Size};
use rikka_compositor_protocols::xdg_dialog_v1::XdgDialogV1;
use rikka_compositor_protocols::xdg_surface::XdgSurface;
use rikka_compositor_protocols::xdg_toplevel::XdgToplevel;
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;

use super::decoration::{Decoration, DecorationPreference};
use super::icon::Icon;
use super::toplevel_configure::{ToplevelConfigure, WmCapabilities};
use super::window::WindowState;
use crate::ids::ToplevelId;
use crate::lifecycle::Lifecycle;

#[derive(Debug)]
pub struct Toplevel {
    pub(crate) resource: XdgToplevel,
    pub(crate) xdg_surface: XdgSurface,
    pub(crate) surface: SurfaceId,
    pub(crate) lifecycle: Lifecycle<ToplevelConfigure, WindowState>,
    pub(crate) title: Option<String>,
    pub(crate) app_id: Option<String>,
    pub(crate) parent: Option<ToplevelId>,
    pub(crate) sent_capabilities: Option<WmCapabilities>,
    pub(crate) decoration: Option<Decoration>,
    pub(crate) dialog: Option<XdgDialogV1>,
    pub(crate) modal: bool,
    pub(crate) icon: Option<Icon>,
    pub(crate) tag: Option<String>,
    pub(crate) description: Option<String>,
}

impl Toplevel {
    pub(crate) fn new(resource: XdgToplevel, xdg_surface: XdgSurface, surface: SurfaceId) -> Self {
        Self {
            resource,
            xdg_surface,
            surface,
            lifecycle: Lifecycle::new(WindowState::default()),
            title: None,
            app_id: None,
            parent: None,
            sent_capabilities: None,
            decoration: None,
            dialog: None,
            modal: false,
            icon: None,
            tag: None,
            description: None,
        }
    }

    pub(crate) fn forget_attributes(&mut self) {
        self.title = None;
        self.app_id = None;
        self.parent = None;
        self.sent_capabilities = None;
        if let Some(decoration) = &mut self.decoration {
            decoration.sent = None;
        }
    }

    pub fn surface(&self) -> SurfaceId {
        self.surface
    }

    pub fn client(&self) -> ClientId {
        self.resource.client()
    }

    pub fn version(&self) -> u32 {
        self.resource.version()
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn app_id(&self) -> Option<&str> {
        self.app_id.as_deref()
    }

    pub fn parent(&self) -> Option<ToplevelId> {
        self.parent
    }

    pub fn window_geometry(&self) -> Option<Rect<i32, Logical>> {
        self.lifecycle.state.geometry
    }

    pub fn min_size(&self) -> Size<i32, Logical> {
        self.lifecycle.state.min_size
    }

    pub fn max_size(&self) -> Size<i32, Logical> {
        self.lifecycle.state.max_size
    }

    pub fn current(&self) -> Option<&ToplevelConfigure> {
        self.lifecycle.current()
    }

    pub fn current_serial(&self) -> Option<u32> {
        self.lifecycle.current_serial()
    }

    pub fn latest(&self) -> Option<&ToplevelConfigure> {
        self.lifecycle.latest()
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

    pub fn decoration(&self) -> DecorationPreference {
        self.decoration
            .as_ref()
            .map_or(DecorationPreference::Absent, |decoration| {
                decoration.preference
            })
    }

    pub fn modal(&self) -> bool {
        self.modal
    }

    pub fn icon(&self) -> Option<&Icon> {
        self.icon.as_ref()
    }

    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}
