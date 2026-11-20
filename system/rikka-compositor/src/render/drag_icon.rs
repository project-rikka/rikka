use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_surface::SurfaceId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DragIcon {
    pub(crate) surface: SurfaceId,
    pub(crate) at: Point<f64, Logical>,
}
