use std::f32::consts::{FRAC_PI_2, TAU};

use rikka_image_edit::Path;
use rikka_types::Point;

use crate::frame::Frame;

const KAPPA: f32 = 0.552_284_8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Rectangle { radius: f32 },
    Ellipse,
    Triangle,
    Polygon { sides: u32 },
    Star { points: u32, depth: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShapeKind {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Triangle,
    Polygon,
    Star,
}

impl ShapeKind {
    pub const ALL: [ShapeKind; 6] = [
        ShapeKind::Rectangle,
        ShapeKind::RoundedRectangle,
        ShapeKind::Ellipse,
        ShapeKind::Triangle,
        ShapeKind::Polygon,
        ShapeKind::Star,
    ];
}

impl From<ShapeKind> for Shape {
    fn from(kind: ShapeKind) -> Shape {
        match kind {
            ShapeKind::Rectangle => Shape::Rectangle { radius: 0.0 },
            ShapeKind::RoundedRectangle => Shape::Rectangle { radius: 24.0 },
            ShapeKind::Ellipse => Shape::Ellipse,
            ShapeKind::Triangle => Shape::Triangle,
            ShapeKind::Polygon => Shape::Polygon { sides: 6 },
            ShapeKind::Star => Shape::Star {
                points: 5,
                depth: Shape::STAR_DEPTH,
            },
        }
    }
}

impl Shape {
    pub const FEWEST_SIDES: u32 = 3;
    pub const MAX_SIDES: u32 = 64;
    pub const SHALLOWEST: f32 = 0.05;
    pub const DEEPEST: f32 = 0.95;
    pub const STAR_DEPTH: f32 = 0.5;

    #[must_use]
    pub fn kind(&self) -> ShapeKind {
        match self {
            Shape::Rectangle { radius } if *radius > 0.0 => ShapeKind::RoundedRectangle,
            Shape::Rectangle { .. } => ShapeKind::Rectangle,
            Shape::Ellipse => ShapeKind::Ellipse,
            Shape::Triangle => ShapeKind::Triangle,
            Shape::Polygon { .. } => ShapeKind::Polygon,
            Shape::Star { .. } => ShapeKind::Star,
        }
    }

    #[must_use]
    pub fn checked(self) -> Shape {
        match self {
            Shape::Rectangle { radius } => Shape::Rectangle {
                radius: match radius.is_finite() {
                    true => radius.max(0.0),
                    false => 0.0,
                },
            },
            Shape::Polygon { sides } => Shape::Polygon {
                sides: sides.clamp(Shape::FEWEST_SIDES, Shape::MAX_SIDES),
            },
            Shape::Star { points, depth } => Shape::Star {
                points: points.clamp(Shape::FEWEST_SIDES, Shape::MAX_SIDES),
                depth: match depth.is_finite() {
                    true => depth.clamp(Shape::SHALLOWEST, Shape::DEEPEST),
                    false => Shape::STAR_DEPTH,
                },
            },
            Shape::Ellipse | Shape::Triangle => self,
        }
    }

    #[must_use]
    pub fn path(&self, frame: &Frame) -> Path {
        match self.checked() {
            Shape::Rectangle { radius } if radius > 0.0 => rounded(frame, radius),
            Shape::Rectangle { .. } => Path::polygon(frame.corners()),
            Shape::Ellipse => ellipse(frame),
            Shape::Triangle => {
                Path::polygon([frame.spot(0, -1), frame.spot(1, 1), frame.spot(-1, 1)])
            }
            Shape::Polygon { sides } => Path::polygon(ring(frame, sides, |_| 1.0)),
            Shape::Star { points, depth } => {
                Path::polygon(ring(frame, points * 2, |at| match at % 2 {
                    0 => 1.0,
                    _ => depth,
                }))
            }
        }
    }
}

fn ring(frame: &Frame, corners: u32, reach: impl Fn(u32) -> f32) -> Vec<Point> {
    let (half_width, half_height) = (frame.width / 2.0, frame.height / 2.0);
    (0..corners)
        .map(|at| {
            let turn = at as f32 / corners as f32 * TAU - FRAC_PI_2;
            let out = reach(at);
            frame.to_world(Point::new(
                half_width * out * turn.cos(),
                half_height * out * turn.sin(),
            ))
        })
        .collect()
}

fn rounded(frame: &Frame, radius: f32) -> Path {
    let (half_width, half_height) = (frame.width / 2.0, frame.height / 2.0);
    let radius = radius.min(half_width).min(half_height);
    let reach = radius * (1.0 - KAPPA);
    let at = |x: f32, y: f32| frame.to_world(Point::new(x, y));
    let (left, right, top, bottom) = (-half_width, half_width, -half_height, half_height);
    Path::builder()
        .move_to(at(left + radius, top))
        .line_to(at(right - radius, top))
        .cubic_to(
            at(right - reach, top),
            at(right, top + reach),
            at(right, top + radius),
        )
        .line_to(at(right, bottom - radius))
        .cubic_to(
            at(right, bottom - reach),
            at(right - reach, bottom),
            at(right - radius, bottom),
        )
        .line_to(at(left + radius, bottom))
        .cubic_to(
            at(left + reach, bottom),
            at(left, bottom - reach),
            at(left, bottom - radius),
        )
        .line_to(at(left, top + radius))
        .cubic_to(
            at(left, top + reach),
            at(left + reach, top),
            at(left + radius, top),
        )
        .close()
        .build()
}

fn ellipse(frame: &Frame) -> Path {
    let (across, down) = (frame.width / 2.0, frame.height / 2.0);
    let at = |x: f32, y: f32| frame.to_world(Point::new(x, y));
    let (reach_across, reach_down) = (across * KAPPA, down * KAPPA);
    Path::builder()
        .move_to(at(across, 0.0))
        .cubic_to(
            at(across, reach_down),
            at(reach_across, down),
            at(0.0, down),
        )
        .cubic_to(
            at(-reach_across, down),
            at(-across, reach_down),
            at(-across, 0.0),
        )
        .cubic_to(
            at(-across, -reach_down),
            at(-reach_across, -down),
            at(0.0, -down),
        )
        .cubic_to(
            at(reach_across, -down),
            at(across, -reach_down),
            at(across, 0.0),
        )
        .close()
        .build()
}
