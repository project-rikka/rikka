use std::ops::RangeInclusive;

use rikka_types::Point;

use crate::animation::Value;
use crate::placement::{finite, finite_point, turned, zoomed};
use crate::project::Format;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CameraField {
    Interest,
    Dolly,
    Zoom,
    Roll,
    Pan,
    Tilt,
    Angle,
}

impl CameraField {
    pub const ALL: [CameraField; 7] = [
        CameraField::Interest,
        CameraField::Dolly,
        CameraField::Zoom,
        CameraField::Roll,
        CameraField::Pan,
        CameraField::Tilt,
        CameraField::Angle,
    ];

    pub(crate) fn is_angle(self) -> bool {
        matches!(
            self,
            CameraField::Roll | CameraField::Pan | CameraField::Tilt
        )
    }

    pub(crate) fn clamp_value(self, value: Value) -> Value {
        match (self, value) {
            (CameraField::Interest, Value::Point(point)) => Value::Point(finite_point(point)),
            (CameraField::Zoom, Value::Scalar(zoom)) => Value::Scalar(zoomed(zoom)),
            (CameraField::Angle, Value::Scalar(angle)) => Value::Scalar(viewed(angle)),
            (_, Value::Scalar(value)) => Value::Scalar(finite(value)),
            (_, other) => other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub interest: Point,
    pub dolly: f32,
    pub zoom: f32,
    pub roll: f32,
    pub pan: f32,
    pub tilt: f32,
    pub angle: f32,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            interest: Point::ZERO,
            dolly: 0.0,
            zoom: 1.0,
            roll: 0.0,
            pan: 0.0,
            tilt: 0.0,
            angle: Camera::ANGLE,
        }
    }
}

impl Camera {
    pub const ANGLE: f32 = 39.6;
    pub const ANGLES: RangeInclusive<f32> = 1.0..=179.0;

    #[must_use]
    pub fn focal(&self, format: Format) -> f64 {
        f64::from(format.width) / 2.0 / (f64::from(self.angle).to_radians() / 2.0).tan()
    }

    #[must_use]
    pub fn dollied(self, distance: impl FnOnce(f64) -> f64, format: Format) -> Camera {
        let focal = self.focal(format);
        let moved = distance(focal - f64::from(self.dolly));
        Camera {
            dolly: (focal - moved) as f32,
            ..self
        }
    }

    #[must_use]
    pub fn checked(self) -> Camera {
        Camera {
            interest: finite_point(self.interest),
            dolly: finite(self.dolly),
            zoom: zoomed(self.zoom),
            roll: turned(self.roll),
            pan: turned(self.pan),
            tilt: turned(self.tilt),
            angle: viewed(self.angle),
        }
    }

    #[must_use]
    pub fn value(&self, field: CameraField) -> Value {
        match field {
            CameraField::Interest => Value::Point(self.interest),
            CameraField::Dolly => Value::Scalar(self.dolly),
            CameraField::Zoom => Value::Scalar(self.zoom),
            CameraField::Roll => Value::Scalar(self.roll),
            CameraField::Pan => Value::Scalar(self.pan),
            CameraField::Tilt => Value::Scalar(self.tilt),
            CameraField::Angle => Value::Scalar(self.angle),
        }
    }

    #[must_use]
    pub fn with(self, field: CameraField, value: Value) -> Camera {
        let mut camera = self;
        match (field, value) {
            (CameraField::Interest, Value::Point(point)) => camera.interest = point,
            (CameraField::Dolly, Value::Scalar(value)) => camera.dolly = value,
            (CameraField::Zoom, Value::Scalar(value)) => camera.zoom = value,
            (CameraField::Roll, Value::Scalar(value)) => camera.roll = value,
            (CameraField::Pan, Value::Scalar(value)) => camera.pan = value,
            (CameraField::Tilt, Value::Scalar(value)) => camera.tilt = value,
            (CameraField::Angle, Value::Scalar(value)) => camera.angle = value,
            _ => {}
        }
        camera
    }
}

fn viewed(angle: f32) -> f32 {
    match angle.is_finite() {
        true => angle.clamp(*Camera::ANGLES.start(), *Camera::ANGLES.end()),
        false => Camera::ANGLE,
    }
}
