use rikka_types::{Point, Rect};

use crate::affine::Affine;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub center: Point,
    pub width: f32,
    pub height: f32,
    pub angle: f32,
}

impl Frame {
    #[must_use]
    pub fn new(center: Point, width: f32, height: f32) -> Frame {
        Frame {
            center,
            width,
            height,
            angle: 0.0,
        }
    }

    #[must_use]
    pub fn from_corners(corner: Point, opposite: Point) -> Frame {
        Frame::new(
            Point::new((corner.x + opposite.x) / 2.0, (corner.y + opposite.y) / 2.0),
            (opposite.x - corner.x).abs(),
            (opposite.y - corner.y).abs(),
        )
    }

    #[must_use]
    pub fn from_rect(rect: Rect) -> Frame {
        Frame::new(rect.center(), rect.width(), rect.height())
    }

    #[must_use]
    pub fn rotated(self, angle: f32) -> Frame {
        Frame { angle, ..self }
    }

    #[must_use]
    pub fn axes(&self) -> (Point, Point) {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        (Point::new(cos, sin), Point::new(-sin, cos))
    }

    #[must_use]
    pub fn to_world(&self, local: Point) -> Point {
        let (across, down) = self.axes();
        self.center + across * local.x + down * local.y
    }

    #[must_use]
    pub fn to_local(&self, point: Point) -> Point {
        let (across, down) = self.axes();
        let offset = point - self.center;
        Point::new(
            offset.x * across.x + offset.y * across.y,
            offset.x * down.x + offset.y * down.y,
        )
    }

    #[must_use]
    pub fn spot(&self, x: i8, y: i8) -> Point {
        self.to_world(Point::new(
            f32::from(x) * self.width / 2.0,
            f32::from(y) * self.height / 2.0,
        ))
    }

    #[must_use]
    pub fn corners(&self) -> [Point; 4] {
        [
            self.spot(-1, -1),
            self.spot(1, -1),
            self.spot(1, 1),
            self.spot(-1, 1),
        ]
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        enclosing(self.corners())
    }

    #[must_use]
    pub fn contains(&self, at: Point, reach: f32) -> bool {
        let local = self.to_local(at);
        local.x.abs() <= self.width / 2.0 + reach && local.y.abs() <= self.height / 2.0 + reach
    }

    #[must_use]
    pub fn mapped(&self, map: Affine) -> Frame {
        let (across, down) = self.axes();
        let reach_across = map.apply_vector(across);
        let reach_down = map.apply_vector(down);
        let length = |at: Point| at.x.hypot(at.y);
        let angle = match length(reach_across) > f32::EPSILON {
            true => reach_across.y.atan2(reach_across.x).to_degrees(),
            false => self.angle,
        };
        Frame {
            center: map.apply(self.center),
            width: self.width * length(reach_across),
            height: self.height * length(reach_down),
            angle,
        }
    }

    #[must_use]
    pub fn placing(&self) -> Affine {
        Affine::rotation(f64::from(self.angle)).then(Affine::translation(
            f64::from(self.center.x),
            f64::from(self.center.y),
        ))
    }
}

#[must_use]
pub fn enclosing(points: impl IntoIterator<Item = Point>) -> Rect {
    let mut points = points.into_iter();
    let Some(first) = points.next() else {
        return Rect::ZERO;
    };
    let (low, high) = points.fold((first, first), |(low, high), at| {
        (low.min(at), high.max(at))
    });
    Rect::new(low.x, low.y, high.x - low.x, high.y - low.y)
}
