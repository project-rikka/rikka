use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_surface::SurfaceId;

use crate::ids::{SnapshotId, WindowId};
use crate::target::Target;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drawn {
    Surface {
        target: Target,
        root: SurfaceId,
        location: Point<i32, Logical>,
    },
    Snapshot {
        window: WindowId,
        snapshot: SnapshotId,
        geometry: Rect<i32, Logical>,
    },
}
