use rikka_figure::{Affine, Frame};
use rikka_types::{Point, Size};

use crate::animation::Value;
use crate::project::Format;

const MIN_SIDE: f32 = 1.0;
const MIN_ZOOM: f32 = 0.01;
const MAX_ZOOM: f32 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PlacementField {
    Position,
    Depth,
    Anchor,
    Zoom,
    Aspect,
    Rotation,
    TiltX,
    TiltY,
    Opacity,
}

impl PlacementField {
    pub const ALL: [PlacementField; 9] = [
        PlacementField::Position,
        PlacementField::Depth,
        PlacementField::Anchor,
        PlacementField::Zoom,
        PlacementField::Aspect,
        PlacementField::Rotation,
        PlacementField::TiltX,
        PlacementField::TiltY,
        PlacementField::Opacity,
    ];

    #[must_use]
    pub fn needs_three_d(self) -> bool {
        matches!(
            self,
            PlacementField::Depth | PlacementField::TiltX | PlacementField::TiltY
        )
    }

    pub(crate) fn is_angle(self) -> bool {
        matches!(
            self,
            PlacementField::Rotation | PlacementField::TiltX | PlacementField::TiltY
        )
    }

    pub(crate) fn clamp_value(self, value: Value) -> Value {
        match (self, value) {
            (PlacementField::Position | PlacementField::Anchor, Value::Point(point)) => {
                Value::Point(finite_point(point))
            }
            (PlacementField::Zoom, Value::Scalar(zoom)) => Value::Scalar(zoomed(zoom)),
            (PlacementField::Aspect, Value::Scalar(aspect)) => Value::Scalar(stretched(aspect)),
            (PlacementField::Opacity, Value::Scalar(opacity)) => Value::Scalar(opaque(opacity)),
            (_, Value::Scalar(value)) => Value::Scalar(finite(value)),
            (_, other) => other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
    pub depth: f32,
    pub anchor: Point,
    pub zoom: f32,
    pub aspect: f32,
    pub rotation: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub opacity: f32,
    pub three_d: bool,
}

impl Default for Placement {
    fn default() -> Placement {
        Placement {
            x: 0.0,
            y: 0.0,
            depth: 0.0,
            anchor: Point::ZERO,
            zoom: 1.0,
            aspect: 0.0,
            rotation: 0.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
            opacity: 1.0,
            three_d: false,
        }
    }
}

impl Placement {
    pub const MIN_ZOOM: f32 = MIN_ZOOM;
    pub const MAX_ZOOM: f32 = MAX_ZOOM;

    #[must_use]
    pub fn checked(self) -> Placement {
        Placement {
            x: finite(self.x),
            y: finite(self.y),
            depth: finite(self.depth),
            anchor: finite_point(self.anchor),
            zoom: zoomed(self.zoom),
            aspect: stretched(self.aspect),
            rotation: turned(self.rotation),
            tilt_x: turned(self.tilt_x),
            tilt_y: turned(self.tilt_y),
            opacity: opaque(self.opacity),
            three_d: self.three_d,
        }
    }

    #[must_use]
    pub fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }

    #[must_use]
    pub fn value(&self, field: PlacementField) -> Value {
        match field {
            PlacementField::Position => Value::Point(self.position()),
            PlacementField::Depth => Value::Scalar(self.depth),
            PlacementField::Anchor => Value::Point(self.anchor),
            PlacementField::Zoom => Value::Scalar(self.zoom),
            PlacementField::Aspect => Value::Scalar(self.aspect),
            PlacementField::Rotation => Value::Scalar(self.rotation),
            PlacementField::TiltX => Value::Scalar(self.tilt_x),
            PlacementField::TiltY => Value::Scalar(self.tilt_y),
            PlacementField::Opacity => Value::Scalar(self.opacity),
        }
    }

    #[must_use]
    pub fn with(self, field: PlacementField, value: Value) -> Placement {
        let mut placement = self;
        match (field, value) {
            (PlacementField::Position, Value::Point(point)) => {
                (placement.x, placement.y) = (point.x, point.y);
            }
            (PlacementField::Anchor, Value::Point(point)) => placement.anchor = point,
            (PlacementField::Depth, Value::Scalar(value)) => placement.depth = value,
            (PlacementField::Zoom, Value::Scalar(value)) => placement.zoom = value,
            (PlacementField::Aspect, Value::Scalar(value)) => placement.aspect = value,
            (PlacementField::Rotation, Value::Scalar(value)) => placement.rotation = value,
            (PlacementField::TiltX, Value::Scalar(value)) => placement.tilt_x = value,
            (PlacementField::TiltY, Value::Scalar(value)) => placement.tilt_y = value,
            (PlacementField::Opacity, Value::Scalar(value)) => placement.opacity = value,
            _ => {}
        }
        placement
    }

    #[must_use]
    pub fn scale(&self) -> (f32, f32) {
        let zoom = self.unzoomed();
        (
            zoom * (1.0 - self.aspect.max(0.0)),
            zoom * (1.0 + self.aspect.min(0.0)),
        )
    }

    #[must_use]
    pub fn center(&self, format: Format) -> Point {
        self.affine(format).apply(Point::ZERO)
    }

    #[must_use]
    pub fn affine(&self, format: Format) -> Affine {
        let (across, down) = self.scale();
        Affine::translation(-f64::from(self.anchor.x), -f64::from(self.anchor.y))
            .then(Affine::scale(f64::from(across), f64::from(down)))
            .then(Affine::rotation(f64::from(self.rotation)))
            .then(Affine::translation(
                f64::from(format.center().x + self.x),
                f64::from(format.center().y + self.y),
            ))
    }

    #[must_use]
    pub fn frame(&self, size: Size, format: Format) -> Frame {
        let (across, down) = self.scale();
        Frame {
            center: self.center(format),
            width: size.width * across,
            height: size.height * down,
            angle: self.rotation,
        }
    }

    #[must_use]
    pub fn moved(self, by: Point) -> Placement {
        Placement {
            x: self.x + by.x,
            y: self.y + by.y,
            ..self
        }
    }

    pub(crate) fn followed(self, after: &Frame, format: Format) -> Placement {
        Placement {
            rotation: turned(after.angle),
            ..self
        }
        .centered_at(after.center, format)
    }

    pub(crate) fn grown(self, before: &Frame, after: &Frame, format: Format) -> Placement {
        let grown = match before.width > f32::EPSILON {
            true => after.width / before.width,
            false => 1.0,
        };
        Placement {
            zoom: (self.zoom.max(MIN_ZOOM) * grown).clamp(MIN_ZOOM, MAX_ZOOM),
            ..self
        }
        .followed(after, format)
    }

    pub(crate) fn centered_at(self, center: Point, format: Format) -> Placement {
        let shift = center - self.center(format);
        self.moved(shift)
    }

    pub(crate) fn unzoomed(&self) -> f32 {
        self.zoom.max(MIN_ZOOM)
    }

    pub(crate) fn least_scale(&self) -> f32 {
        let (across, down) = self.scale();
        across.min(down).max(MIN_ZOOM)
    }
}

pub(crate) fn finite(value: f32) -> f32 {
    match value.is_finite() {
        true => value,
        false => 0.0,
    }
}

pub(crate) fn finite_point(point: Point) -> Point {
    Point::new(finite(point.x), finite(point.y))
}

pub(crate) fn side(value: f32) -> f32 {
    match value.is_finite() {
        true => value.max(MIN_SIDE),
        false => MIN_SIDE,
    }
}

pub(crate) fn turned(degrees: f32) -> f32 {
    if !degrees.is_finite() {
        return 0.0;
    }
    let wrapped = degrees.rem_euclid(360.0);
    match wrapped > 180.0 {
        true => wrapped - 360.0,
        false => wrapped,
    }
}

pub(crate) fn zoomed(zoom: f32) -> f32 {
    match zoom.is_finite() {
        true => zoom.clamp(MIN_ZOOM, MAX_ZOOM),
        false => 1.0,
    }
}

fn stretched(aspect: f32) -> f32 {
    match aspect.is_finite() {
        true => aspect.clamp(-1.0, 1.0),
        false => 0.0,
    }
}

fn opaque(opacity: f32) -> f32 {
    match opacity.is_finite() {
        true => opacity.clamp(0.0, 1.0),
        false => 1.0,
    }
}
