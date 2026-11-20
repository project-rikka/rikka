use rikka_types::Transform;

use crate::{Coordinate, Point, Rect, Size};

impl<N: Coordinate, S> Size<N, S> {
    pub fn transformed(self, transform: Transform) -> Self {
        match transform.swaps_axes() {
            true => Size::new(self.h, self.w),
            false => self,
        }
    }
}

impl<N: Coordinate, S> Point<N, S> {
    pub fn transformed_in(self, transform: Transform, area: &Size<N, S>) -> Self {
        let axes = transform.axis_map();
        let mapped = area.transformed(transform);
        let (x, y) = match axes.swapped {
            true => (self.y, self.x),
            false => (self.x, self.y),
        };
        Point::new(
            if axes.mirrored_x { mapped.w - x } else { x },
            if axes.mirrored_y { mapped.h - y } else { y },
        )
    }
}

impl<N: Coordinate, S> Rect<N, S> {
    pub fn transformed_in(self, transform: Transform, area: &Size<N, S>) -> Self {
        let near = self.loc.transformed_in(transform, area);
        let far = (self.loc + self.size).transformed_in(transform, area);
        Rect::from_extremities(
            (near.x.min(far.x), near.y.min(far.y)),
            (near.x.max(far.x), near.y.max(far.y)),
        )
    }
}
