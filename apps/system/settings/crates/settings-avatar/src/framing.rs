use rikka_image::Region;
use rikka_types::{Point, Size};

pub const MAX_ZOOM: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    photo: Size,
    zoom: f32,
    center: Point,
}

impl Framing {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Framing {
        let photo = Size::new(width as f32, height as f32);
        Framing {
            photo,
            zoom: 1.0,
            center: Point::new(photo.width / 2.0, photo.height / 2.0),
        }
    }

    #[must_use]
    pub fn photo(&self) -> Size {
        self.photo
    }

    #[must_use]
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    #[must_use]
    pub fn center(&self) -> Point {
        self.center
    }

    #[must_use]
    pub fn side(&self) -> f32 {
        self.photo.min_side() / self.zoom
    }

    #[must_use]
    pub fn panned(self, by: Point) -> Framing {
        Framing {
            center: Point::new(self.center.x + by.x, self.center.y + by.y),
            ..self
        }
        .clamped()
    }

    #[must_use]
    pub fn zoomed(self, zoom: f32, anchor: Point) -> Framing {
        let zoom = zoom.clamp(1.0, MAX_ZOOM);
        let shrink = self.zoom / zoom;
        Framing {
            zoom,
            center: Point::new(
                anchor.x - (anchor.x - self.center.x) * shrink,
                anchor.y - (anchor.y - self.center.y) * shrink,
            ),
            ..self
        }
        .clamped()
    }

    #[must_use]
    pub fn region(&self) -> Region {
        let side = (self.side().round() as u32).clamp(1, self.photo.min_side() as u32);
        let start = |center: f32, extent: f32| {
            let room = extent as u32 - side;
            ((center - side as f32 / 2.0).round().max(0.0) as u32).min(room)
        };
        Region::new(
            start(self.center.x, self.photo.width),
            start(self.center.y, self.photo.height),
            side,
            side,
        )
    }

    fn clamped(self) -> Framing {
        let half = self.side() / 2.0;
        let within = |center: f32, extent: f32| center.clamp(half, extent - half);
        Framing {
            center: Point::new(
                within(self.center.x, self.photo.width),
                within(self.center.y, self.photo.height),
            ),
            ..self
        }
    }
}
