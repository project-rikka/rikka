use crate::{Coordinate, Point, Rect, Size};

impl<N: Coordinate, S> From<Point<N, S>> for rikka_types::Point {
    fn from(point: Point<N, S>) -> Self {
        Self::new(point.x.to_f64() as f32, point.y.to_f64() as f32)
    }
}

impl<N: Coordinate, S> From<Size<N, S>> for rikka_types::Size {
    fn from(size: Size<N, S>) -> Self {
        Self::new(size.w.to_f64() as f32, size.h.to_f64() as f32)
    }
}

impl<N: Coordinate, S> From<Rect<N, S>> for rikka_types::Rect {
    fn from(rect: Rect<N, S>) -> Self {
        Self::from_origin_size(rect.loc.into(), rect.size.into())
    }
}

impl<S> Rect<i32, S> {
    pub fn covering(rect: rikka_types::Rect) -> Self {
        let rounded = rect.round_out();
        Self::new(
            Point::new(rounded.min_x() as i32, rounded.min_y() as i32),
            Size::new(rounded.width() as i32, rounded.height() as i32),
        )
    }
}
