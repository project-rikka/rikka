use std::f32::consts::PI;

use rikka_types::Point;

use crate::marks::path::Path;
use crate::marks::path_mark::{Cap, Join, StrokeStyle};

const ROUND_TOLERANCE: f32 = 0.1;
const MIN_ROUND_SEGMENTS: usize = 8;
const MAX_ROUND_SEGMENTS: usize = 256;
const SAME: f32 = 1e-4;

pub(crate) fn outlined(path: &Path, style: StrokeStyle) -> Vec<Vec<Point>> {
    let half = style.width / 2.0;
    if half.is_nan() || half <= 0.0 {
        return Vec::new();
    }
    let mut pieces = Vec::new();
    for subpath in path.subpaths() {
        let points = distinct(subpath.points());
        match points.as_slice() {
            [] => {}
            [only] => dot(&mut pieces, *only, half, style.cap),
            _ => line(&mut pieces, &points, subpath.is_closed(), half, style),
        }
    }
    pieces
        .into_iter()
        .map(|mut piece| {
            if area(&piece) < 0.0 {
                piece.reverse();
            }
            piece
        })
        .collect()
}

fn line(
    pieces: &mut Vec<Vec<Point>>,
    points: &[Point],
    closed: bool,
    half: f32,
    style: StrokeStyle,
) {
    let count = points.len();
    let segments = match closed {
        true => count,
        false => count - 1,
    };
    for at in 0..segments {
        let (from, to) = (points[at], points[(at + 1) % count]);
        let across = left(direction(from, to)) * half;
        pieces.push(vec![from + across, to + across, to - across, from - across]);
    }
    let corners = match closed {
        true => 0..count,
        false => 1..count - 1,
    };
    for at in corners {
        let before = points[(at + count - 1) % count];
        let (here, after) = (points[at], points[(at + 1) % count]);
        join(pieces, before, here, after, half, style);
    }
    if !closed {
        cap(
            pieces,
            points[0],
            direction(points[1], points[0]),
            half,
            style.cap,
        );
        cap(
            pieces,
            points[count - 1],
            direction(points[count - 2], points[count - 1]),
            half,
            style.cap,
        );
    }
}

fn join(
    pieces: &mut Vec<Vec<Point>>,
    before: Point,
    here: Point,
    after: Point,
    half: f32,
    style: StrokeStyle,
) {
    let (incoming, outgoing) = (direction(before, here), direction(here, after));
    let turn = incoming.x * outgoing.y - incoming.y * outgoing.x;
    let along = incoming.x * outgoing.x + incoming.y * outgoing.y;
    if turn.abs() < SAME && along > 0.0 {
        return;
    }
    if style.join == Join::Round {
        pieces.push(circle(here, half));
        return;
    }
    let side = if turn > 0.0 { -1.0 } else { 1.0 };
    let first = left(incoming) * side;
    let second = left(outgoing) * side;
    let (start, end) = (here + first * half, here + second * half);
    let ratio = 1.0 / ((1.0 + along) / 2.0).max(0.0).sqrt();
    if style.join == Join::Miter && ratio <= style.miter_limit && ratio.is_finite() {
        let bisector = unit(first + second);
        let reach = half / (bisector.x * first.x + bisector.y * first.y);
        pieces.push(vec![here, start, here + bisector * reach, end]);
        return;
    }
    pieces.push(vec![here, start, end]);
}

fn cap(pieces: &mut Vec<Vec<Point>>, at: Point, outward: Point, half: f32, cap: Cap) {
    match cap {
        Cap::Butt => {}
        Cap::Round => pieces.push(circle(at, half)),
        Cap::Square => {
            let across = left(outward) * half;
            let beyond = at + outward * half;
            pieces.push(vec![
                at + across,
                beyond + across,
                beyond - across,
                at - across,
            ]);
        }
    }
}

fn dot(pieces: &mut Vec<Vec<Point>>, at: Point, half: f32, cap: Cap) {
    match cap {
        Cap::Butt => {}
        Cap::Round => pieces.push(circle(at, half)),
        Cap::Square => pieces.push(vec![
            Point::new(at.x - half, at.y - half),
            Point::new(at.x + half, at.y - half),
            Point::new(at.x + half, at.y + half),
            Point::new(at.x - half, at.y + half),
        ]),
    }
}

fn circle(center: Point, radius: f32) -> Vec<Point> {
    let step = (1.0 - ROUND_TOLERANCE / radius).max(-1.0).acos();
    let count = match step > 0.0 {
        true => ((PI / step).ceil() as usize).clamp(MIN_ROUND_SEGMENTS, MAX_ROUND_SEGMENTS),
        false => MAX_ROUND_SEGMENTS,
    };
    (0..count)
        .map(|at| {
            let turn = at as f32 / count as f32 * std::f32::consts::TAU;
            Point::new(
                center.x + radius * turn.cos(),
                center.y + radius * turn.sin(),
            )
        })
        .collect()
}

fn distinct(points: &[Point]) -> Vec<Point> {
    let mut unique: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        if unique
            .last()
            .is_none_or(|last| last.distance(*point) > SAME)
        {
            unique.push(*point);
        }
    }
    if unique.len() > 1 && unique[0].distance(unique[unique.len() - 1]) <= SAME {
        unique.pop();
    }
    unique
}

fn direction(from: Point, to: Point) -> Point {
    unit(to - from)
}

fn unit(vector: Point) -> Point {
    let length = vector.x.hypot(vector.y);
    match length > 0.0 {
        true => vector * (1.0 / length),
        false => Point::new(1.0, 0.0),
    }
}

fn left(direction: Point) -> Point {
    Point::new(-direction.y, direction.x)
}

pub(crate) fn area(polygon: &[Point]) -> f32 {
    let count = polygon.len();
    (0..count)
        .map(|at| {
            let (a, b) = (polygon[at], polygon[(at + 1) % count]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        / 2.0
}
