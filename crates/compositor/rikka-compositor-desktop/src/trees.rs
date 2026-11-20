use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_surface::{SurfaceId, Surfaces};

pub trait SurfaceTrees {
    fn surface_under(
        &self,
        root: SurfaceId,
        point: Point<f64, Logical>,
    ) -> Option<(SurfaceId, Point<f64, Logical>)>;

    fn bounding_box(&self, root: SurfaceId) -> Option<Rect<i32, Logical>>;
}

impl SurfaceTrees for Surfaces {
    fn surface_under(
        &self,
        root: SurfaceId,
        point: Point<f64, Logical>,
    ) -> Option<(SurfaceId, Point<f64, Logical>)> {
        Surfaces::surface_under(self, root, point)
    }

    fn bounding_box(&self, root: SurfaceId) -> Option<Rect<i32, Logical>> {
        Surfaces::bounding_box(self, root)
    }
}
