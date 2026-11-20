use crate::blend::BlendMode;
use crate::color::Color;
use crate::descriptor::{Descriptor, Id, Unit, Value};

pub(crate) fn number(descriptor: &Descriptor, key: &str, fallback: f64) -> f64 {
    descriptor.number(key).unwrap_or(fallback)
}

pub(crate) fn percent(descriptor: &Descriptor, key: &str, fallback: f64) -> f64 {
    descriptor
        .number(key)
        .map_or(fallback, |value| value / 100.0)
}

pub(crate) fn flag(descriptor: &Descriptor, key: &str, fallback: bool) -> bool {
    descriptor.bool(key).unwrap_or(fallback)
}

pub(crate) fn blend_mode(descriptor: &Descriptor, key: &str) -> BlendMode {
    descriptor
        .enumeration(key)
        .map_or(BlendMode::Normal, |value| {
            BlendMode::from_descriptor_value(&value)
        })
}

pub(crate) fn color(descriptor: &Descriptor, key: &str) -> Option<Color> {
    descriptor.descriptor(key).and_then(Color::from_descriptor)
}

pub(crate) fn point(descriptor: &Descriptor, key: &str) -> (f64, f64) {
    descriptor.descriptor(key).map_or((0.0, 0.0), |point| {
        (number(point, "Hrzn", 0.0), number(point, "Vrtc", 0.0))
    })
}

pub(crate) fn enumeration(kind: &str, value: &str) -> Value {
    Value::Enum(Id::from(kind), Id::from(value))
}

pub(crate) fn percent_value(value: f64) -> Value {
    Value::UnitDouble(Unit::PERCENT, value * 100.0)
}

pub(crate) fn pixels(value: f64) -> Value {
    Value::UnitDouble(Unit::PIXELS, value)
}

pub(crate) fn angle(value: f64) -> Value {
    Value::UnitDouble(Unit::ANGLE, value)
}

pub(crate) fn blend_value(mode: BlendMode) -> Value {
    enumeration("BlnM", mode.descriptor_value())
}

pub(crate) fn point_value(x: f64, y: f64, unit: Unit) -> Value {
    Value::Descriptor(
        Descriptor::new("Pnt ")
            .with("Hrzn", Value::UnitDouble(unit, x))
            .with("Vrtc", Value::UnitDouble(unit, y)),
    )
}

pub(crate) fn color_value(color: &Color) -> Value {
    Value::Descriptor(color.to_descriptor())
}
