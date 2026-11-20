use rikka_types::{Point, Rect};

use crate::affine::Affine;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LetterWipe {
    pub from: Point,
    pub progress: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LetterPose {
    pub offset: Point,
    pub zoom: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub blur: f32,
    pub wipe: Option<LetterWipe>,
}

impl Default for LetterPose {
    fn default() -> LetterPose {
        LetterPose::REST
    }
}

impl LetterPose {
    pub const REST: LetterPose = LetterPose {
        offset: Point::ZERO,
        zoom: 1.0,
        rotation: 0.0,
        opacity: 1.0,
        blur: 0.0,
        wipe: None,
    };

    #[must_use]
    pub fn is_rest(&self) -> bool {
        *self == LetterPose::REST
    }

    pub(crate) fn is_hidden(&self) -> bool {
        self.opacity <= 0.0
            || self.zoom <= 0.0
            || self.wipe.is_some_and(|wipe| wipe.progress <= 0.0)
    }

    pub(crate) fn map(&self, centre: Point, scale: f32) -> Affine {
        let zoom = f64::from(self.zoom);
        Affine::about(
            centre,
            Affine::scale(zoom, zoom).then(Affine::rotation(f64::from(self.rotation))),
        )
        .then(Affine::translation(
            f64::from(self.offset.x * scale),
            f64::from(self.offset.y * scale),
        ))
    }
}

impl LetterWipe {
    #[must_use]
    pub fn cut(&self, polygon: &[Point], across: Rect) -> Vec<Point> {
        let reach = |point: Point| point.x * self.from.x + point.y * self.from.y;
        let spans = across.corners().map(reach);
        let low = spans.iter().copied().fold(f32::INFINITY, f32::min);
        let high = spans.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let threshold = high - self.progress.clamp(0.0, 1.0) * (high - low);
        clipped(polygon, |point| reach(point) - threshold)
    }

    pub(crate) fn clipped_polygon(&self, ink: Rect, around: Rect) -> Vec<Point> {
        self.cut(&around.corners(), ink)
    }
}

fn clipped(polygon: &[Point], side: impl Fn(Point) -> f32) -> Vec<Point> {
    let mut clipped = Vec::with_capacity(polygon.len() + 2);
    for (index, &current) in polygon.iter().enumerate() {
        let previous = polygon[(index + polygon.len() - 1) % polygon.len()];
        let (was, is) = (side(previous), side(current));
        if (was >= 0.0) != (is >= 0.0) {
            clipped.push(previous + (current - previous) * (was / (was - is)));
        }
        if is >= 0.0 {
            clipped.push(current);
        }
    }
    clipped
}
