use std::sync::Arc;

use rikka_types::Point;

const TOLERANCE: f32 = 0.2;
const MIN_TOLERANCE: f32 = 0.01;
const MAX_SEGMENTS: f32 = 1024.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Subpath {
    points: Vec<Point>,
    closed: bool,
}

impl Subpath {
    #[must_use]
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    fn scaled(&self, factor: f32) -> Subpath {
        Subpath {
            points: self.points.iter().map(|point| *point * factor).collect(),
            closed: self.closed,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    subpaths: Arc<[Subpath]>,
}

impl Path {
    pub fn builder() -> PathBuilder {
        PathBuilder::default()
    }

    #[must_use]
    pub fn polyline(points: impl IntoIterator<Item = Point>) -> Path {
        Path::from_points(points, false)
    }

    #[must_use]
    pub fn polygon(points: impl IntoIterator<Item = Point>) -> Path {
        Path::from_points(points, true)
    }

    fn from_points(points: impl IntoIterator<Item = Point>, closed: bool) -> Path {
        let mut builder = PathBuilder::default();
        for (at, point) in points.into_iter().enumerate() {
            builder = match at {
                0 => builder.move_to(point),
                _ => builder.line_to(point),
            };
        }
        match closed {
            true => builder.close().build(),
            false => builder.build(),
        }
    }

    #[must_use]
    pub fn subpaths(&self) -> &[Subpath] {
        &self.subpaths
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.subpaths.is_empty()
    }

    #[must_use]
    pub(crate) fn scaled(&self, factor: f32) -> Path {
        Path {
            subpaths: self
                .subpaths
                .iter()
                .map(|subpath| subpath.scaled(factor))
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
#[must_use]
pub struct PathBuilder {
    finished: Vec<Subpath>,
    open: Vec<Point>,
    tolerance: f32,
}

impl Default for PathBuilder {
    fn default() -> PathBuilder {
        PathBuilder {
            finished: Vec::new(),
            open: Vec::new(),
            tolerance: TOLERANCE,
        }
    }
}

impl PathBuilder {
    pub fn tolerance(mut self, pixels: f32) -> PathBuilder {
        self.tolerance = pixels.max(MIN_TOLERANCE);
        self
    }

    pub fn move_to(mut self, to: Point) -> PathBuilder {
        self.end(false);
        self.open.push(to);
        self
    }

    pub fn line_to(mut self, to: Point) -> PathBuilder {
        self.reach(to);
        self
    }

    pub fn quad_to(mut self, control: Point, to: Point) -> PathBuilder {
        let from = self.current(control);
        let bend = (from - control * 2.0 + to).length();
        let pieces = pieces((bend / (4.0 * self.tolerance)).sqrt());
        for step in 1..=pieces {
            let t = step as f32 / pieces as f32;
            let u = 1.0 - t;
            self.reach(from * (u * u) + control * (2.0 * u * t) + to * (t * t));
        }
        self
    }

    pub fn cubic_to(mut self, first: Point, second: Point, to: Point) -> PathBuilder {
        let from = self.current(first);
        let bend = (from - first * 2.0 + second)
            .length()
            .max((first - second * 2.0 + to).length());
        let pieces = pieces((0.75 * bend / self.tolerance).sqrt());
        for step in 1..=pieces {
            let t = step as f32 / pieces as f32;
            let u = 1.0 - t;
            self.reach(
                from * (u * u * u)
                    + first * (3.0 * u * u * t)
                    + second * (3.0 * u * t * t)
                    + to * (t * t * t),
            );
        }
        self
    }

    pub fn close(mut self) -> PathBuilder {
        let start = self.open.first().copied();
        self.end(true);
        if let Some(start) = start {
            self.open.push(start);
        }
        self
    }

    #[must_use]
    pub fn build(mut self) -> Path {
        self.end(false);
        Path {
            subpaths: self.finished.into(),
        }
    }

    fn current(&mut self, otherwise: Point) -> Point {
        match self.open.last() {
            Some(last) => *last,
            None => {
                self.open.push(otherwise);
                otherwise
            }
        }
    }

    fn reach(&mut self, to: Point) {
        if self.open.last() != Some(&to) {
            self.open.push(to);
        }
    }

    fn end(&mut self, closed: bool) {
        let mut points = std::mem::take(&mut self.open);
        if closed && points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        let drawn = points.len() > 1 || (closed && !points.is_empty());
        if drawn {
            self.finished.push(Subpath { points, closed });
        }
    }
}

trait Length {
    fn length(self) -> f32;
}

impl Length for Point {
    fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
}

fn pieces(estimate: f32) -> usize {
    match estimate.is_finite() {
        true => estimate.ceil().clamp(1.0, MAX_SEGMENTS) as usize,
        false => 1,
    }
}
