use rikka_blur::rectangles;
use rikka_compositor_geometry::{Logical, Point, Rect, Region, Size};
use rikka_types::{CornerRadii, Rect as SceneRect, RoundedRect, Size as SceneSize};

pub(super) type Step = Rect<i32, Logical>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Radii {
    pub(super) top_left: i32,
    pub(super) top_right: i32,
    pub(super) bottom_right: i32,
    pub(super) bottom_left: i32,
}

impl Radii {
    pub(super) const SQUARE: Radii = Radii {
        top_left: 0,
        top_right: 0,
        bottom_right: 0,
        bottom_left: 0,
    };

    fn largest(&self) -> i32 {
        self.top_left
            .max(self.top_right)
            .max(self.bottom_right)
            .max(self.bottom_left)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Shape {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) right: i32,
    pub(super) bottom: i32,
    pub(super) radii: Radii,
}

impl Shape {
    pub(super) fn width(&self) -> i32 {
        self.right - self.left
    }

    fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub(super) fn rounds_within_itself(&self) -> bool {
        2 * self.radii.largest() <= self.width().min(self.height())
    }

    pub(super) fn steps(&self) -> impl Iterator<Item = Step> {
        let mut raster = Vec::new();
        rectangles(self.rounded(), &mut raster);
        raster
            .into_iter()
            .map(|[x, y, width, height]| Rect::new(Point::new(x, y), Size::new(width, height)))
    }

    pub(super) fn region(&self) -> Region<Logical> {
        self.steps().collect()
    }

    pub(super) fn area(&self) -> i64 {
        self.steps()
            .map(|step| i64::from(step.size.w) * i64::from(step.size.h))
            .sum()
    }

    pub(super) fn rounded(&self) -> RoundedRect {
        let (width, height) = (self.width() as f32, self.height() as f32);
        let radii = self.radii;
        RoundedRect::new(
            SceneRect::new(self.left as f32, self.top as f32, width, height),
            CornerRadii::new(
                radii.top_left as f32,
                radii.top_right as f32,
                radii.bottom_right as f32,
                radii.bottom_left as f32,
            )
            .clamp_for_size(SceneSize::new(width, height)),
        )
    }
}

pub(super) fn cut(radius: i32, distance: i32) -> i32 {
    if radius <= 0 || distance >= radius {
        return 0;
    }
    let (radius, from_centre) = (radius as f32, (radius - distance) as f32);
    let across = (radius * radius - from_centre * from_centre)
        .max(0.0)
        .sqrt();
    (radius - across).ceil() as i32
}
