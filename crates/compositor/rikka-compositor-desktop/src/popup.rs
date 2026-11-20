use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_shell::{LayerId, PopupId};
use rikka_compositor_surface::SurfaceId;

use crate::ids::WindowId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Parent {
    Window(WindowId),
    Layer(LayerId),
    Popup(PopupId),
}

#[derive(Debug, Clone)]
pub struct Popup {
    pub(crate) surface: SurfaceId,
    pub(crate) parent: Parent,
    pub(crate) geometry: Rect<i32, Logical>,
    pub(crate) inset: Point<i32, Logical>,
    pub(crate) children: Vec<PopupId>,
}

impl Popup {
    pub(crate) fn new(surface: SurfaceId, parent: Parent) -> Self {
        Self {
            surface,
            parent,
            geometry: Rect::zero(),
            inset: Point::zero(),
            children: Vec::new(),
        }
    }

    pub fn surface(&self) -> SurfaceId {
        self.surface
    }

    pub fn parent(&self) -> Parent {
        self.parent
    }

    pub fn geometry(&self) -> Rect<i32, Logical> {
        self.geometry
    }

    pub fn inset(&self) -> Point<i32, Logical> {
        self.inset
    }

    pub fn children(&self) -> &[PopupId] {
        &self.children
    }

    pub fn set_geometry(&mut self, geometry: Rect<i32, Logical>) {
        self.geometry = geometry;
    }

    pub fn set_inset(&mut self, inset: Point<i32, Logical>) {
        self.inset = inset;
    }
}
