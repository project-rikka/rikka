use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_shell::ToplevelId;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::XWindowId;

use crate::window::WindowKind;
use crate::workspace::Workspace;

#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct NewWindow {
    pub(crate) kind: WindowKind,
    pub(crate) surface: Option<SurfaceId>,
    pub(crate) geometry: Rect<i32, Logical>,
    pub(crate) workspace: Option<Workspace>,
    pub(crate) unmanaged: bool,
}

impl NewWindow {
    pub fn xdg(toplevel: ToplevelId, surface: SurfaceId, geometry: Rect<i32, Logical>) -> Self {
        Self::new(WindowKind::Xdg(toplevel), Some(surface), geometry)
    }

    pub fn x11(window: XWindowId, geometry: Rect<i32, Logical>) -> Self {
        Self::new(WindowKind::X11(window), None, geometry)
    }

    fn new(kind: WindowKind, surface: Option<SurfaceId>, geometry: Rect<i32, Logical>) -> Self {
        Self {
            kind,
            surface,
            geometry,
            workspace: None,
            unmanaged: false,
        }
    }

    pub fn with_workspace(mut self, workspace: Workspace) -> Self {
        self.workspace = Some(workspace);
        self
    }

    pub fn unmanaged(mut self) -> Self {
        self.unmanaged = true;
        self
    }
}
