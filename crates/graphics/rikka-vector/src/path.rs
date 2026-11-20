use std::ops::Range;

use rikka_types::{Point, Rect};

use crate::transform::Transform;

const MAX_POINTS: usize = 1 << 20;
const CIRCLE_CONTROL: f32 = 0.552_284_8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Move,
    Line,
    Quad,
    Cubic,
    Close,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    verbs: Vec<Verb>,
    points: Vec<Point>,
    contour_starts: Vec<usize>,
    closes: usize,
    reopens: bool,
    truncated: bool,
}

impl Path {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.verbs.clear();
        self.points.clear();
        self.contour_starts.clear();
        self.closes = 0;
        self.reopens = false;
        self.truncated = false;
    }

    #[inline]
    #[must_use]
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verbs.len() <= self.contour_starts.len() + self.closes
    }

    #[inline]
    #[must_use]
    pub fn verbs(&self) -> &[Verb] {
        &self.verbs
    }

    #[inline]
    #[must_use]
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    #[inline]
    pub fn points_mut(&mut self) -> &mut [Point] {
        &mut self.points
    }

    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.points.capacity() * size_of::<Point>()
            + self.contour_starts.capacity() * size_of::<usize>()
            + self.verbs.capacity()
    }

    pub fn contours(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        let total = self.points.len();
        self.contour_starts
            .iter()
            .enumerate()
            .map(move |(index, &start)| {
                let end = self.contour_starts.get(index + 1).copied().unwrap_or(total);
                start..end
            })
    }

    #[must_use]
    pub fn segments(&self) -> Segments<'_> {
        Segments {
            path: self,
            verb: 0,
            point: 0,
            start: Point::default(),
            current: Point::default(),
            phase: Phase::Idle,
        }
    }

    #[inline]
    pub fn move_to(&mut self, to: Point) {
        if self.is_full(1) {
            return;
        }
        self.reopens = false;
        self.contour_starts.push(self.points.len());
        self.verbs.push(Verb::Move);
        self.points.push(to);
    }

    #[inline]
    pub fn line_to(&mut self, to: Point) {
        if self.is_full(1) {
            return;
        }
        self.ensure_contour();
        self.verbs.push(Verb::Line);
        self.points.push(to);
    }

    #[inline]
    pub fn quad_to(&mut self, control: Point, to: Point) {
        if self.is_full(2) {
            return;
        }
        self.ensure_contour();
        self.verbs.push(Verb::Quad);
        self.points.push(control);
        self.points.push(to);
    }

    #[inline]
    pub fn cubic_to(&mut self, first: Point, second: Point, to: Point) {
        if self.is_full(3) {
            return;
        }
        self.ensure_contour();
        self.verbs.push(Verb::Cubic);
        self.points.push(first);
        self.points.push(second);
        self.points.push(to);
    }

    pub fn close(&mut self) {
        if self.contour_starts.is_empty() || self.reopens {
            return;
        }
        self.verbs.push(Verb::Close);
        self.closes += 1;
        self.reopens = true;
    }

    #[must_use]
    pub fn current(&self) -> Option<Point> {
        match self.reopens {
            true => self.contour_starts.last().map(|&start| self.points[start]),
            false => self.points.last().copied(),
        }
    }

    pub fn add_rect(&mut self, rect: Rect) {
        self.move_to(Point::new(rect.min_x(), rect.min_y()));
        self.line_to(Point::new(rect.max_x(), rect.min_y()));
        self.line_to(Point::new(rect.max_x(), rect.max_y()));
        self.line_to(Point::new(rect.min_x(), rect.max_y()));
        self.close();
    }

    pub fn add_rounded_rect(&mut self, rect: Rect, radii: Point) {
        let rx = radii.x.min(rect.width() * 0.5);
        let ry = radii.y.min(rect.height() * 0.5);
        if rx <= 0.0 || ry <= 0.0 {
            self.add_rect(rect);
            return;
        }
        let (x0, y0, x1, y1) = (rect.min_x(), rect.min_y(), rect.max_x(), rect.max_y());
        let (kx, ky) = (rx * (1.0 - CIRCLE_CONTROL), ry * (1.0 - CIRCLE_CONTROL));
        let at = Point::new;
        self.move_to(at(x0 + rx, y0));
        self.line_to(at(x1 - rx, y0));
        self.cubic_to(at(x1 - kx, y0), at(x1, y0 + ky), at(x1, y0 + ry));
        self.line_to(at(x1, y1 - ry));
        self.cubic_to(at(x1, y1 - ky), at(x1 - kx, y1), at(x1 - rx, y1));
        self.line_to(at(x0 + rx, y1));
        self.cubic_to(at(x0 + kx, y1), at(x0, y1 - ky), at(x0, y1 - ry));
        self.line_to(at(x0, y0 + ry));
        self.cubic_to(at(x0, y0 + ky), at(x0 + kx, y0), at(x0 + rx, y0));
        self.close();
    }

    pub fn add_ellipse(&mut self, center: Point, radii: Point) {
        let (cx, cy, rx, ry) = (center.x, center.y, radii.x, radii.y);
        let (kx, ky) = (rx * CIRCLE_CONTROL, ry * CIRCLE_CONTROL);
        let at = Point::new;
        self.move_to(at(cx + rx, cy));
        self.cubic_to(at(cx + rx, cy + ky), at(cx + kx, cy + ry), at(cx, cy + ry));
        self.cubic_to(at(cx - kx, cy + ry), at(cx - rx, cy + ky), at(cx - rx, cy));
        self.cubic_to(at(cx - rx, cy - ky), at(cx - kx, cy - ry), at(cx, cy - ry));
        self.cubic_to(at(cx + kx, cy - ry), at(cx + rx, cy - ky), at(cx + rx, cy));
        self.close();
    }

    #[must_use]
    pub fn transformed(&self, transform: &Transform) -> Path {
        let mut placed = self.clone();
        for point in &mut placed.points {
            *point = transform.apply(*point);
        }
        placed
    }

    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        let mut extent = Extent::EMPTY;
        let mut drawn = false;
        for segment in self.segments() {
            match segment {
                Segment::Begin(_) | Segment::End => {}
                Segment::Line(from, to) => {
                    extent.include(from);
                    extent.include(to);
                    drawn = true;
                }
                Segment::Quad(from, control, to) => {
                    extent.include(from);
                    extent.include(to);
                    for axis in [Axis::X, Axis::Y] {
                        let (a, b, c) = (axis.of(from), axis.of(control), axis.of(to));
                        let bend = a - 2.0 * b + c;
                        if bend != 0.0 {
                            let t = (a - b) / bend;
                            if t > 0.0 && t < 1.0 {
                                let u = 1.0 - t;
                                extent.include_axis(axis, u * u * a + 2.0 * u * t * b + t * t * c);
                            }
                        }
                    }
                    drawn = true;
                }
                Segment::Cubic(from, first, second, to) => {
                    extent.include(from);
                    extent.include(to);
                    for axis in [Axis::X, Axis::Y] {
                        let p = [axis.of(from), axis.of(first), axis.of(second), axis.of(to)];
                        for t in cubic_extrema(p) {
                            let u = 1.0 - t;
                            let value = u * u * u * p[0]
                                + 3.0 * u * u * t * p[1]
                                + 3.0 * u * t * t * p[2]
                                + t * t * t * p[3];
                            extent.include_axis(axis, value);
                        }
                    }
                    drawn = true;
                }
            }
        }
        if !drawn {
            let only = self.points.first().copied()?;
            return Some(Rect::new(only.x, only.y, 0.0, 0.0));
        }
        extent.rect()
    }

    #[inline]
    fn is_full(&mut self, points: usize) -> bool {
        if self.points.len() + points + 1 > MAX_POINTS {
            self.truncated = true;
        }
        self.truncated
    }

    #[inline]
    fn ensure_contour(&mut self) {
        match self.contour_starts.last() {
            None => self.move_to(Point::ZERO),
            Some(&start) if self.reopens => self.move_to(self.points[start]),
            Some(_) => {}
        }
    }
}

fn cubic_extrema(p: [f32; 4]) -> impl Iterator<Item = f32> {
    let a = -p[0] + 3.0 * p[1] - 3.0 * p[2] + p[3];
    let b = 2.0 * (p[0] - 2.0 * p[1] + p[2]);
    let c = p[1] - p[0];
    let mut roots = [f32::NAN; 2];
    if a.abs() < 1.0e-12 {
        if b != 0.0 {
            roots[0] = -c / b;
        }
    } else {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant >= 0.0 {
            let root = discriminant.sqrt();
            roots = [(-b + root) / (2.0 * a), (-b - root) / (2.0 * a)];
        }
    }
    roots.into_iter().filter(|t| *t > 0.0 && *t < 1.0)
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

impl Axis {
    fn of(self, point: Point) -> f32 {
        match self {
            Axis::X => point.x,
            Axis::Y => point.y,
        }
    }
}

struct Extent {
    low: Point,
    high: Point,
}

impl Extent {
    const EMPTY: Extent = Extent {
        low: Point::new(f32::INFINITY, f32::INFINITY),
        high: Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
    };

    fn include(&mut self, point: Point) {
        self.low = self.low.min(point);
        self.high = self.high.max(point);
    }

    fn include_axis(&mut self, axis: Axis, value: f32) {
        match axis {
            Axis::X => {
                self.low.x = self.low.x.min(value);
                self.high.x = self.high.x.max(value);
            }
            Axis::Y => {
                self.low.y = self.low.y.min(value);
                self.high.y = self.high.y.max(value);
            }
        }
    }

    fn rect(&self) -> Option<Rect> {
        let finite = (self.low.x + self.low.y + self.high.x + self.high.y).is_finite();
        finite.then(|| {
            Rect::new(
                self.low.x,
                self.low.y,
                self.high.x - self.low.x,
                self.high.y - self.low.y,
            )
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    Begin(Point),
    Line(Point, Point),
    Quad(Point, Point, Point),
    Cubic(Point, Point, Point, Point),
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Drawing,
    Closed,
}

#[derive(Clone, Debug)]
pub struct Segments<'p> {
    path: &'p Path,
    verb: usize,
    point: usize,
    start: Point,
    current: Point,
    phase: Phase,
}

impl Iterator for Segments<'_> {
    type Item = Segment;

    fn next(&mut self) -> Option<Segment> {
        let points = &self.path.points;
        while self.path.verbs.get(self.verb) == Some(&Verb::Close) {
            self.verb += 1;
        }
        let verb = self.path.verbs.get(self.verb).copied();
        let ends_contour = matches!(verb, None | Some(Verb::Move));
        if ends_contour && self.phase == Phase::Drawing {
            self.phase = Phase::Closed;
            if self.current != self.start {
                let closing = Segment::Line(self.current, self.start);
                self.current = self.start;
                return Some(closing);
            }
        }
        if ends_contour && self.phase == Phase::Closed {
            self.phase = Phase::Idle;
            return Some(Segment::End);
        }
        let verb = verb?;
        self.verb += 1;
        let from = self.current;
        let segment = match verb {
            Verb::Move => {
                self.start = points[self.point];
                self.current = self.start;
                self.point += 1;
                self.phase = Phase::Drawing;
                return Some(Segment::Begin(self.start));
            }
            Verb::Line => {
                let to = points[self.point];
                self.point += 1;
                self.current = to;
                Segment::Line(from, to)
            }
            Verb::Quad => {
                let (control, to) = (points[self.point], points[self.point + 1]);
                self.point += 2;
                self.current = to;
                Segment::Quad(from, control, to)
            }
            Verb::Close => return self.next(),
            Verb::Cubic => {
                let (a, b, to) = (
                    points[self.point],
                    points[self.point + 1],
                    points[self.point + 2],
                );
                self.point += 3;
                self.current = to;
                Segment::Cubic(from, a, b, to)
            }
        };
        Some(segment)
    }
}
