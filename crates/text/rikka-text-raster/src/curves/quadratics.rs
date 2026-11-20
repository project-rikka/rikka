use rikka_text_font::{Point, Transform};

use rikka_vector::Segment;

use crate::outline::path::Outline;

const CUBIC_ERROR_FACTOR: f32 = 0.048_112_52;
const MAX_PIECES: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Quadratic {
    pub(crate) p0: Point,
    pub(crate) p1: Point,
    pub(crate) p2: Point,
}

pub(crate) fn quadratics(
    outline: &Outline,
    transform: &Transform,
    tolerance: f32,
    out: &mut Vec<Quadratic>,
) {
    out.clear();
    for segment in outline.path().segments() {
        match segment {
            Segment::Line(from, to) => {
                let (p0, p2) = (transform.apply(from), transform.apply(to));
                if p0 != p2 {
                    out.push(Quadratic {
                        p0,
                        p1: Point::new((p0.x + p2.x) * 0.5, (p0.y + p2.y) * 0.5),
                        p2,
                    });
                }
            }
            Segment::Quad(from, control, to) => out.push(Quadratic {
                p0: transform.apply(from),
                p1: transform.apply(control),
                p2: transform.apply(to),
            }),
            Segment::Cubic(from, a, b, to) => cubic(
                [
                    transform.apply(from),
                    transform.apply(a),
                    transform.apply(b),
                    transform.apply(to),
                ],
                tolerance,
                out,
            ),
            Segment::Begin(_) | Segment::End => {}
        }
    }
}

fn cubic(points: [Point; 4], tolerance: f32, out: &mut Vec<Quadratic>) {
    let [p0, p1, p2, p3] = points;
    let third = Point::new(
        p3.x - 3.0 * p2.x + 3.0 * p1.x - p0.x,
        p3.y - 3.0 * p2.y + 3.0 * p1.y - p0.y,
    );
    let error = (third.x * third.x + third.y * third.y).sqrt() * CUBIC_ERROR_FACTOR;
    let pieces = (error / tolerance)
        .cbrt()
        .ceil()
        .clamp(1.0, MAX_PIECES as f32) as u32;
    let at = |t: f32| -> (Point, Point) {
        let u = 1.0 - t;
        let position = Point::new(
            u * u * u * p0.x + 3.0 * u * u * t * p1.x + 3.0 * u * t * t * p2.x + t * t * t * p3.x,
            u * u * u * p0.y + 3.0 * u * u * t * p1.y + 3.0 * u * t * t * p2.y + t * t * t * p3.y,
        );
        let tangent = Point::new(
            3.0 * (u * u * (p1.x - p0.x) + 2.0 * u * t * (p2.x - p1.x) + t * t * (p3.x - p2.x)),
            3.0 * (u * u * (p1.y - p0.y) + 2.0 * u * t * (p2.y - p1.y) + t * t * (p3.y - p2.y)),
        );
        (position, tangent)
    };
    let step = 1.0 / pieces as f32;
    let (mut start, mut start_tangent) = (p0, at(0.0).1);
    for piece in 1..=pieces {
        let (end, end_tangent) = if piece == pieces {
            (p3, at(1.0).1)
        } else {
            at(piece as f32 * step)
        };
        let scale = step / 3.0;
        let c1 = Point::new(
            start.x + start_tangent.x * scale,
            start.y + start_tangent.y * scale,
        );
        let c2 = Point::new(end.x - end_tangent.x * scale, end.y - end_tangent.y * scale);
        let control = Point::new(
            (3.0 * (c1.x + c2.x) - start.x - end.x) * 0.25,
            (3.0 * (c1.y + c2.y) - start.y - end.y) * 0.25,
        );
        if start != end {
            out.push(Quadratic {
                p0: start,
                p1: control,
                p2: end,
            });
        }
        start = end;
        start_tangent = end_tangent;
    }
}
