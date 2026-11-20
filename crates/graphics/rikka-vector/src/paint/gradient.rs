use rikka_types::{Color, Point};

use crate::transform::Transform;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub offset: f32,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Spread {
    #[default]
    Pad,
    Repeat,
    Reflect,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Interpolation {
    #[default]
    Srgb,
    LinearLight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientShape {
    Linear {
        start: Point,
        end: Point,
    },
    Radial {
        start_center: Point,
        start_radius: f32,
        end_center: Point,
        end_radius: f32,
    },
    Sweep {
        center: Point,
        start_degrees: f32,
        end_degrees: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient<'a> {
    pub shape: GradientShape,
    pub stops: &'a [Stop],
    pub spread: Spread,
    pub interpolation: Interpolation,
    pub transform: Transform,
}
