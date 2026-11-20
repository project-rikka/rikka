use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_shell::ToplevelId;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::OutputId;
use rikka_compositor_types::XWindowId;

use crate::frame::Frame;
use crate::tags::Tags;
use crate::visibility::Visibility;
use crate::workspace::Workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowKind {
    Xdg(ToplevelId),
    X11(XWindowId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Placement {
    #[default]
    Floating,
    Tiled,
    Maximized,
    Fullscreen,
}

#[derive(Debug)]
pub struct Window {
    pub(crate) kind: WindowKind,
    pub(crate) surface: Option<SurfaceId>,
    pub(crate) geometry: Rect<i32, Logical>,
    pub(crate) inset: Point<i32, Logical>,
    pub(crate) frame: Frame,
    pub(crate) placement: Placement,
    pub(crate) tags: Tags,
    pub(crate) visibility: Visibility,
    pub(crate) workspace: Option<Workspace>,
    pub(crate) unmanaged: bool,
    pub(crate) confined: bool,
}

impl Window {
    pub(crate) fn new(
        kind: WindowKind,
        surface: Option<SurfaceId>,
        geometry: Rect<i32, Logical>,
        unmanaged: bool,
    ) -> Self {
        Self {
            kind,
            surface,
            geometry,
            inset: Point::zero(),
            frame: Frame::NONE,
            placement: Placement::default(),
            tags: Tags::empty(),
            visibility: Visibility::Shown,
            workspace: None,
            unmanaged,
            confined: false,
        }
    }

    pub fn kind(&self) -> WindowKind {
        self.kind
    }

    pub fn surface(&self) -> Option<SurfaceId> {
        self.surface
    }

    pub fn geometry(&self) -> Rect<i32, Logical> {
        self.geometry
    }

    pub fn inset(&self) -> Point<i32, Logical> {
        self.inset
    }

    pub fn frame(&self) -> Frame {
        self.frame
    }

    pub fn content(&self) -> Rect<i32, Logical> {
        self.frame.content(self.geometry)
    }

    pub fn surface_location(&self) -> Point<i32, Logical> {
        self.geometry.loc + self.frame.offset() - self.inset
    }

    pub fn placement(&self) -> Placement {
        self.placement
    }

    pub fn tags(&self) -> Tags {
        self.tags
    }

    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub fn workspace(&self) -> Option<Workspace> {
        self.workspace
    }

    pub fn output(&self) -> Option<OutputId> {
        self.workspace.map(|workspace| workspace.output())
    }

    pub fn unmanaged(&self) -> bool {
        self.unmanaged
    }

    pub fn set_geometry(&mut self, geometry: Rect<i32, Logical>) {
        self.geometry = geometry;
    }

    pub fn set_inset(&mut self, inset: Point<i32, Logical>) {
        self.inset = inset;
    }

    pub fn set_frame(&mut self, frame: Frame) {
        let content = self.frame.content_size(self.geometry.size);
        self.geometry.size = frame.outer_size(content);
        self.frame = frame;
    }

    pub fn set_placement(&mut self, placement: Placement) {
        self.placement = placement;
    }

    pub fn set_tags(&mut self, tags: Tags) {
        self.tags = tags;
    }

    pub fn set_confined(&mut self, confined: bool) {
        self.confined = confined;
    }

    pub(crate) fn drawable(&self) -> bool {
        matches!(self.visibility, Visibility::Shown | Visibility::Exiting(_))
    }
}
