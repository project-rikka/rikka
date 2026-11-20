use rikka_types::{Point, Rect, Size};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    whole: Rect,
    shown: Rect,
}

impl Projection {
    #[must_use]
    pub fn new(whole: Rect, shown: Rect) -> Projection {
        Projection { whole, shown }
    }

    #[must_use]
    pub fn scale(&self) -> f32 {
        match self.whole.width() > 0.0 {
            true => self.shown.width() / self.whole.width(),
            false => 0.0,
        }
    }

    #[must_use]
    pub fn point(&self, desk: Point) -> Point {
        let scale = self.scale();
        Point::new(
            self.shown.min_x() + (desk.x - self.whole.min_x()) * scale,
            self.shown.min_y() + (desk.y - self.whole.min_y()) * scale,
        )
    }

    #[must_use]
    pub fn rect(&self, desk: Rect) -> Rect {
        let scale = self.scale();
        Rect::from_origin_size(
            self.point(desk.origin),
            Size::new(desk.width() * scale, desk.height() * scale),
        )
    }

    #[must_use]
    pub fn back(&self, shown: Point) -> Point {
        let scale = self.scale();
        if scale <= 0.0 {
            return self.whole.origin;
        }
        Point::new(
            self.whole.min_x() + (shown.x - self.shown.min_x()) / scale,
            self.whole.min_y() + (shown.y - self.shown.min_y()) / scale,
        )
    }

    #[must_use]
    pub fn back_length(&self, shown: f32) -> f32 {
        let scale = self.scale();
        match scale > 0.0 {
            true => shown / scale,
            false => 0.0,
        }
    }
}
