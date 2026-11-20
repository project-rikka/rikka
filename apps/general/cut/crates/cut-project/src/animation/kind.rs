use std::fmt::Debug;

use rikka_types::{Color, Point};

use super::key::Bend;
use super::oklab;

pub(crate) type Coords = [f64; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Scalar(f32),
    Point(Point),
    Color(Color),
}

impl Value {
    #[must_use]
    pub fn scalar(self) -> Option<f32> {
        match self {
            Value::Scalar(value) => Some(value),
            Value::Point(_) | Value::Color(_) => None,
        }
    }

    #[must_use]
    pub fn point(self) -> Option<Point> {
        match self {
            Value::Point(point) => Some(point),
            Value::Scalar(_) | Value::Color(_) => None,
        }
    }

    #[must_use]
    pub fn color(self) -> Option<Color> {
        match self {
            Value::Color(color) => Some(color),
            Value::Scalar(_) | Value::Point(_) => None,
        }
    }

    pub(crate) fn differs(self, other: Value) -> bool {
        const APART: f32 = 1e-4;
        let apart = |left: f32, right: f32| (left - right).abs() > APART;
        match (self, other) {
            (Value::Scalar(left), Value::Scalar(right)) => apart(left, right),
            (Value::Point(left), Value::Point(right)) => {
                apart(left.x, right.x) || apart(left.y, right.y)
            }
            (Value::Color(left), Value::Color(right)) => {
                apart(left.r, right.r)
                    || apart(left.g, right.g)
                    || apart(left.b, right.b)
                    || apart(left.a, right.a)
            }
            _ => true,
        }
    }
}

pub trait Kind: Copy + Debug + Default + PartialEq + Send + Sync + 'static {
    type Value: Copy + Debug + PartialEq + Send + Sync + 'static;
    type Bend: Copy + Debug + Default + PartialEq + Send + Sync + 'static;

    const DIMENSIONS: usize;

    fn coords(value: Self::Value) -> [f64; 4];

    fn from_coords(coords: [f64; 4]) -> Self::Value;

    fn bend(bend: &Self::Bend) -> Option<&Bend>;

    fn bend_mut(bend: &mut Self::Bend) -> Option<&mut Bend>;

    fn erase(value: Self::Value) -> Value;

    fn restore(value: Value) -> Option<Self::Value>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Scalar;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Planar;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Spatial;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Chroma;

impl Kind for Scalar {
    type Value = f32;
    type Bend = ();

    const DIMENSIONS: usize = 1;

    fn coords(value: f32) -> Coords {
        [f64::from(value), 0.0, 0.0, 0.0]
    }

    fn from_coords(coords: Coords) -> f32 {
        coords[0] as f32
    }

    fn bend(_: &()) -> Option<&Bend> {
        None
    }

    fn bend_mut(_: &mut ()) -> Option<&mut Bend> {
        None
    }

    fn erase(value: f32) -> Value {
        Value::Scalar(value)
    }

    fn restore(value: Value) -> Option<f32> {
        value.scalar()
    }
}

impl Kind for Planar {
    type Value = Point;
    type Bend = ();

    const DIMENSIONS: usize = 2;

    fn coords(value: Point) -> Coords {
        point_coords(value)
    }

    fn from_coords(coords: Coords) -> Point {
        Point::new(coords[0] as f32, coords[1] as f32)
    }

    fn bend(_: &()) -> Option<&Bend> {
        None
    }

    fn bend_mut(_: &mut ()) -> Option<&mut Bend> {
        None
    }

    fn erase(value: Point) -> Value {
        Value::Point(value)
    }

    fn restore(value: Value) -> Option<Point> {
        value.point()
    }
}

impl Kind for Spatial {
    type Value = Point;
    type Bend = Bend;

    const DIMENSIONS: usize = 2;

    fn coords(value: Point) -> Coords {
        point_coords(value)
    }

    fn from_coords(coords: Coords) -> Point {
        Point::new(coords[0] as f32, coords[1] as f32)
    }

    fn bend(bend: &Bend) -> Option<&Bend> {
        Some(bend)
    }

    fn bend_mut(bend: &mut Bend) -> Option<&mut Bend> {
        Some(bend)
    }

    fn erase(value: Point) -> Value {
        Value::Point(value)
    }

    fn restore(value: Value) -> Option<Point> {
        value.point()
    }
}

impl Kind for Chroma {
    type Value = Color;
    type Bend = ();

    const DIMENSIONS: usize = 4;

    fn coords(value: Color) -> Coords {
        oklab::from_color(value)
    }

    fn from_coords(coords: Coords) -> Color {
        oklab::to_color(coords)
    }

    fn bend(_: &()) -> Option<&Bend> {
        None
    }

    fn bend_mut(_: &mut ()) -> Option<&mut Bend> {
        None
    }

    fn erase(value: Color) -> Value {
        Value::Color(value)
    }

    fn restore(value: Value) -> Option<Color> {
        value.color()
    }
}

pub(crate) fn finite<K: Kind>(value: K::Value) -> bool {
    K::coords(value).iter().all(|coord| coord.is_finite())
}

fn point_coords(value: Point) -> Coords {
    [f64::from(value.x), f64::from(value.y), 0.0, 0.0]
}
