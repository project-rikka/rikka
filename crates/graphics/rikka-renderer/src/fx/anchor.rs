use rikka_types::{Point, Rect};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Anchor {
    origin: Point,
    scale: f32,
}

impl Anchor {
    pub(crate) fn new(origin: Point, scale: f32) -> Anchor {
        Anchor { origin, scale }
    }

    pub(crate) fn origin(&self) -> Point {
        self.origin
    }

    pub(crate) fn scale(&self) -> f32 {
        self.scale
    }

    pub(crate) fn on(&self, scale: f32) -> Anchor {
        Anchor {
            origin: self.origin,
            scale,
        }
    }

    pub(crate) fn point(&self, point: Point) -> Point {
        Point::new(
            (point.x - self.origin.x) * self.scale,
            (point.y - self.origin.y) * self.scale,
        )
    }

    pub(crate) fn rect(&self, rect: Rect) -> Rect {
        let min = self.point(Point::new(rect.min_x(), rect.min_y()));
        let max = self.point(Point::new(rect.max_x(), rect.max_y()));
        Rect::new(min.x, min.y, max.x - min.x, max.y - min.y).round_out()
    }

    pub(crate) fn scene_rect(&self, texels: Rect) -> Rect {
        let inv = self.scale.recip();
        Rect::new(
            texels.min_x() * inv + self.origin.x,
            texels.min_y() * inv + self.origin.y,
            texels.width() * inv,
            texels.height() * inv,
        )
    }
}
