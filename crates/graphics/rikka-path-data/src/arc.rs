use std::f32::consts::{FRAC_PI_2, TAU};

use rikka_types::Point;

use crate::segments::Segment;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Arc {
    pub(crate) from: Point,
    pub(crate) radii: Point,
    pub(crate) rotation: f32,
    pub(crate) large: bool,
    pub(crate) sweep: bool,
    pub(crate) to: Point,
}

impl Arc {
    pub(crate) fn segments(self) -> Vec<Segment> {
        let Arc {
            from,
            radii,
            rotation,
            large,
            sweep,
            to,
        } = self;
        let (rx, ry) = (radii.x.abs(), radii.y.abs());
        if from == to {
            return Vec::new();
        }
        if rx == 0.0 || ry == 0.0 || !(rx + ry).is_finite() {
            return vec![Segment::LineTo(to)];
        }
        let (sin, cos) = rotation.to_radians().sin_cos();
        let half = Point::new((from.x - to.x) * 0.5, (from.y - to.y) * 0.5);
        let x1 = cos * half.x + sin * half.y;
        let y1 = -sin * half.x + cos * half.y;
        let spread = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
        let (rx, ry) = match spread > 1.0 {
            true => (rx * spread.sqrt(), ry * spread.sqrt()),
            false => (rx, ry),
        };
        let numerator = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
        let denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
        let mut factor = match denominator > 0.0 {
            true => (numerator / denominator).max(0.0).sqrt(),
            false => 0.0,
        };
        if large == sweep {
            factor = -factor;
        }
        let cx1 = factor * rx * y1 / ry;
        let cy1 = -factor * ry * x1 / rx;
        let center = Point::new(
            cos * cx1 - sin * cy1 + (from.x + to.x) * 0.5,
            sin * cx1 + cos * cy1 + (from.y + to.y) * 0.5,
        );
        let start = ((y1 - cy1) / ry).atan2((x1 - cx1) / rx);
        let end = ((-y1 - cy1) / ry).atan2((-x1 - cx1) / rx);
        let mut span = end - start;
        if sweep && span < 0.0 {
            span += TAU;
        } else if !sweep && span > 0.0 {
            span -= TAU;
        }
        let pieces = (span.abs() / FRAC_PI_2).ceil().max(1.0) as usize;
        let step = span / pieces as f32;
        let handle = 4.0 / 3.0 * (step * 0.25).tan();
        let place = |unit: Point| {
            Point::new(
                center.x + cos * rx * unit.x - sin * ry * unit.y,
                center.y + sin * rx * unit.x + cos * ry * unit.y,
            )
        };
        let mut angle = start;
        (0..pieces)
            .map(|piece| {
                let next = angle + step;
                let (sin0, cos0) = angle.sin_cos();
                let (sin1, cos1) = next.sin_cos();
                let first = place(Point::new(cos0 - handle * sin0, sin0 + handle * cos0));
                let second = place(Point::new(cos1 + handle * sin1, sin1 - handle * cos1));
                let end = match piece + 1 == pieces {
                    true => to,
                    false => place(Point::new(cos1, sin1)),
                };
                angle = next;
                Segment::CubicTo(first, second, end)
            })
            .collect()
    }
}
