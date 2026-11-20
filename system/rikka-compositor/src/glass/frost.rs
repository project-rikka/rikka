use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_output::blur_region;
use rikka_compositor_panes::read_panes;
use rikka_compositor_surface::{SurfaceId, Surfaces};
use rikka_types::{Rect as SceneRect, RoundedRect};

use super::pane_shapes::next_key;

pub(crate) type Frost = Vec<(u64, RoundedRect)>;

pub(crate) fn frosted(
    surfaces: &Surfaces,
    root: SurfaceId,
    offset: Point<i32, Logical>,
) -> Vec<RoundedRect> {
    let (Some(region), Some(size)) = (
        blur_region(surfaces, root),
        surfaces.get(root).and_then(|surface| surface.size()),
    ) else {
        return Vec::new();
    };
    let mut shapes = Vec::new();
    read_panes(region, size, |shape| {
        let rect = shape.rect;
        let moved = SceneRect::new(
            rect.x() + offset.x as f32,
            rect.y() + offset.y as f32,
            rect.width(),
            rect.height(),
        );
        shapes.push(RoundedRect::new(moved, shape.radii));
    });
    shapes
}

pub(crate) fn frost(shapes: &[RoundedRect]) -> Frost {
    shapes.iter().map(|shape| (next_key(), *shape)).collect()
}
