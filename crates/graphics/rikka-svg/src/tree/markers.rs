use std::f32::consts::{FRAC_PI_2, TAU};

use rikka_types::Point;
use rikka_vector::{Path, Verb};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Orientation {
    Auto,
    AutoStartReverse,
    Angle(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Position {
    Start,
    Middle,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Vertex {
    pub(crate) point: Point,
    pub(crate) angle: f32,
}

#[derive(Clone, Copy, Debug)]
enum Step {
    Move(Point),
    Line(Point),
    Cubic(Point, Point, Point),
    Close,
}

pub(crate) fn orientation(value: Option<&str>) -> Orientation {
    match value.map(str::trim) {
        Some("auto") => Orientation::Auto,
        Some("auto-start-reverse") => Orientation::AutoStartReverse,
        Some(value) => Orientation::Angle(angle(value).unwrap_or(0.0)),
        None => Orientation::Angle(0.0),
    }
}

fn angle(value: &str) -> Option<f32> {
    let units = [
        ("deg", 1.0),
        ("grad", 0.9),
        ("rad", 180.0 / std::f32::consts::PI),
        ("turn", 360.0),
    ];
    for (suffix, scale) in units {
        if let Some(number) = value.strip_suffix(suffix) {
            return number
                .trim()
                .parse::<f32>()
                .ok()
                .map(|number| number * scale);
        }
    }
    value.parse().ok()
}

pub(crate) fn vertices(path: &Path, position: Position, orientation: Orientation) -> Vec<Vertex> {
    let steps = steps(path);
    if steps.len() < 2 {
        return Vec::new();
    }
    let angle_at = |index: usize| match orientation {
        Orientation::AutoStartReverse if index == 0 => (vertex_angle(&steps, 0) + 180.0) % 360.0,
        Orientation::Auto | Orientation::AutoStartReverse => vertex_angle(&steps, index),
        Orientation::Angle(angle) => angle,
    };
    let last = steps.len() - 1;
    let mut found = Vec::new();
    match position {
        Position::Start => {
            if let Step::Move(point) = steps[0] {
                found.push(Vertex {
                    point,
                    angle: angle_at(0),
                });
            }
        }
        Position::Middle => {
            for (index, step) in steps.iter().enumerate().take(last).skip(1) {
                let point = match *step {
                    Step::Move(point) | Step::Line(point) | Step::Cubic(_, _, point) => point,
                    Step::Close => continue,
                };
                found.push(Vertex {
                    point,
                    angle: angle_at(index),
                });
            }
        }
        Position::End => {
            let point = match steps[last] {
                Step::Line(point) | Step::Cubic(_, _, point) => Some(point),
                Step::Close => Some(subpath_start(&steps, last)),
                Step::Move(_) => None,
            };
            if let Some(point) = point {
                found.push(Vertex {
                    point,
                    angle: angle_at(last),
                });
            }
        }
    }
    found
}

fn steps(path: &Path) -> Vec<Step> {
    let points = path.points();
    let mut steps = Vec::with_capacity(path.verbs().len());
    let (mut at, mut previous, mut start) = (0, Point::default(), Point::default());
    for verb in path.verbs() {
        match verb {
            Verb::Move => {
                previous = points[at];
                start = previous;
                steps.push(Step::Move(previous));
                at += 1;
            }
            Verb::Line => {
                previous = points[at];
                steps.push(Step::Line(previous));
                at += 1;
            }
            Verb::Quad => {
                let (control, to) = (points[at], points[at + 1]);
                let first = previous + (control - previous) * (2.0 / 3.0);
                let second = to + (control - to) * (2.0 / 3.0);
                steps.push(Step::Cubic(first, second, to));
                previous = to;
                at += 2;
            }
            Verb::Cubic => {
                previous = points[at + 2];
                steps.push(Step::Cubic(points[at], points[at + 1], previous));
                at += 3;
            }
            Verb::Close => {
                steps.push(Step::Close);
                previous = start;
            }
        }
    }
    steps
}

fn subpath_start(steps: &[Step], index: usize) -> Point {
    steps[..index]
        .iter()
        .rev()
        .find_map(|step| match step {
            Step::Move(point) => Some(*point),
            _ => None,
        })
        .unwrap_or_default()
}

fn previous_vertex(steps: &[Step], index: usize) -> Point {
    match steps[index - 1] {
        Step::Move(point) | Step::Line(point) | Step::Cubic(_, _, point) => point,
        Step::Close => subpath_start(steps, index),
    }
}

fn near(one: Point, other: Point) -> bool {
    (one.x - other.x).abs() <= f32::EPSILON * one.x.abs().max(1.0)
        && (one.y - other.y).abs() <= f32::EPSILON * one.y.abs().max(1.0)
}

fn vertex_angle(steps: &[Step], index: usize) -> f32 {
    let last = steps.len() - 1;
    if index == 0 {
        return match (steps[0], steps[1]) {
            (Step::Move(from), Step::Line(to)) => line_angle(from, to),
            (Step::Move(from), Step::Cubic(control, _, to)) => match near(from, control) {
                true => line_angle(from, to),
                false => line_angle(from, control),
            },
            _ => 0.0,
        };
    }
    if index == last {
        return match (steps[index - 1], steps[index]) {
            (_, Step::Move(_)) => 0.0,
            (_, Step::Line(to)) => line_angle(previous_vertex(steps, index), to),
            (_, Step::Cubic(first, second, to)) => match near(second, to) {
                true => line_angle(first, to),
                false => line_angle(second, to),
            },
            (Step::Line(point), Step::Close) => line_angle(point, subpath_start(steps, index)),
            (Step::Cubic(_, second, point), Step::Close) => {
                let next = subpath_start(steps, index);
                curves_angle(previous_vertex(steps, index), second, point, next, next)
            }
            (_, Step::Close) => 0.0,
        };
    }
    let previous = || previous_vertex(steps, index);
    match (steps[index], steps[index + 1]) {
        (Step::Move(from), Step::Line(to)) => line_angle(from, to),
        (Step::Move(from), Step::Cubic(control, _, _)) => line_angle(from, control),
        (Step::Line(point), Step::Line(next)) => bisector(previous(), point, point, next),
        (Step::Cubic(_, second, point), Step::Cubic(first, _, next)) => {
            curves_angle(previous(), second, point, first, next)
        }
        (Step::Line(point), Step::Cubic(first, _, next)) => {
            let previous = previous();
            curves_angle(previous, previous, point, first, next)
        }
        (Step::Cubic(_, second, point), Step::Line(next)) => {
            curves_angle(previous(), second, point, next, next)
        }
        (Step::Line(point), Step::Move(_)) => line_angle(previous(), point),
        (Step::Cubic(_, second, point), Step::Move(_)) => match near(point, second) {
            true => line_angle(previous(), point),
            false => line_angle(second, point),
        },
        (Step::Line(point), Step::Close) => {
            bisector(previous(), point, point, subpath_start(steps, index))
        }
        (_, Step::Close) => line_angle(previous(), subpath_start(steps, index)),
        (_, Step::Move(_)) | (Step::Close, _) => 0.0,
    }
}

fn line_angle(from: Point, to: Point) -> f32 {
    bisector(from, to, from, to)
}

fn curves_angle(
    previous: Point,
    incoming: Point,
    point: Point,
    outgoing: Point,
    next: Point,
) -> f32 {
    if near(incoming, point) {
        bisector(previous, point, point, outgoing)
    } else if near(point, outgoing) {
        bisector(incoming, point, point, next)
    } else {
        bisector(incoming, point, point, outgoing)
    }
}

fn bisector(from: Point, to: Point, start: Point, end: Point) -> f32 {
    let normalized = |radians: f32| {
        let value = radians % TAU;
        if value < 0.0 { value + TAU } else { value }
    };
    let direction = |delta: Point| {
        let radians = delta.y.atan2(delta.x);
        if radians.is_nan() {
            0.0
        } else {
            normalized(radians)
        }
    };
    let incoming = direction(to - from);
    let outgoing = direction(end - start);
    let half = (outgoing - incoming) * 0.5;
    let mut angle = incoming + half;
    if half.abs() > FRAC_PI_2 {
        angle -= std::f32::consts::PI;
    }
    normalized(angle).to_degrees()
}
