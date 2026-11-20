use rikka_compositor_desktop::{SnapshotId, WindowId};
use rikka_compositor_surface::SurfaceId;

use crate::wayland::ViewId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Painted {
    Surface(SurfaceId, Option<ViewId>),
    Caption(SurfaceId, Option<ViewId>),
    Edge(SurfaceId, Option<ViewId>),
    Outline(WindowId),
    Frozen(SnapshotId, usize),
    PanelBeneath(usize),
    PanelOver(usize),
}
