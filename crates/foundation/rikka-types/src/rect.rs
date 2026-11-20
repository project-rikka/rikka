use crate::{Point, Size};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Self = Self {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    #[inline]
    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    #[inline]
    pub fn from_center(center: Point, size: Size) -> Self {
        Self::new(
            center.x - size.width / 2.0,
            center.y - size.height / 2.0,
            size.width,
            size.height,
        )
    }

    #[inline]
    pub fn from_circle(center: Point, radius: f32) -> Self {
        Self::from_center(center, Size::new(radius * 2.0, radius * 2.0))
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.origin.x
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.origin.y
    }

    #[inline]
    pub fn width(&self) -> f32 {
        self.size.width
    }

    #[inline]
    pub fn height(&self) -> f32 {
        self.size.height
    }

    #[inline]
    pub fn min_x(&self) -> f32 {
        self.origin.x
    }

    #[inline]
    pub fn min_y(&self) -> f32 {
        self.origin.y
    }

    #[inline]
    pub fn max_x(&self) -> f32 {
        self.origin.x + self.size.width
    }

    #[inline]
    pub fn max_y(&self) -> f32 {
        self.origin.y + self.size.height
    }

    #[inline]
    pub fn center(&self) -> Point {
        Point::new(
            self.origin.x + self.size.width * 0.5,
            self.origin.y + self.size.height * 0.5,
        )
    }

    #[inline]
    pub fn corners(&self) -> [Point; 4] {
        [
            Point::new(self.min_x(), self.min_y()),
            Point::new(self.max_x(), self.min_y()),
            Point::new(self.max_x(), self.max_y()),
            Point::new(self.min_x(), self.max_y()),
        ]
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.size.is_empty()
    }

    #[inline]
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.min_x()
            && point.x < self.max_x()
            && point.y >= self.min_y()
            && point.y < self.max_y()
    }

    pub fn nearest(&self, point: Point) -> Point {
        Point::new(
            point.x.clamp(self.min_x(), self.max_x().max(self.min_x())),
            point.y.clamp(self.min_y(), self.max_y().max(self.min_y())),
        )
    }

    pub fn contains_rect(&self, other: &Self) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.min_x() <= other.min_x()
            && self.min_y() <= other.min_y()
            && self.max_x() >= other.max_x()
            && self.max_y() >= other.max_y()
    }

    #[inline]
    pub fn intersects(&self, other: &Self) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.min_x() < other.max_x()
            && other.min_x() < self.max_x()
            && self.min_y() < other.max_y()
            && other.min_y() < self.max_y()
    }

    #[inline]
    pub fn touches(&self, other: &Self) -> bool {
        self.min_x() <= other.max_x()
            && other.min_x() <= self.max_x()
            && self.min_y() <= other.max_y()
            && other.min_y() <= self.max_y()
    }

    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let min_x = self.min_x().max(other.min_x());
        let min_y = self.min_y().max(other.min_y());
        let max_x = self.max_x().min(other.max_x());
        let max_y = self.max_y().min(other.max_y());
        if min_x < max_x && min_y < max_y {
            Some(Self::new(min_x, min_y, max_x - min_x, max_y - min_y))
        } else {
            None
        }
    }

    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let min_x = self.min_x().min(other.min_x());
        let min_y = self.min_y().min(other.min_y());
        let max_x = self.max_x().max(other.max_x());
        let max_y = self.max_y().max(other.max_y());
        Self::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    pub fn inflate(&self, amount: f32) -> Self {
        Self::new(
            self.origin.x - amount,
            self.origin.y - amount,
            (self.size.width + amount * 2.0).max(0.0),
            (self.size.height + amount * 2.0).max(0.0),
        )
    }

    #[inline]
    pub fn translate(&self, delta: Point) -> Self {
        Self::from_origin_size(self.origin + delta, self.size)
    }

    #[inline]
    pub fn scale(&self, factor: f32) -> Self {
        Self::new(
            self.origin.x * factor,
            self.origin.y * factor,
            self.size.width * factor,
            self.size.height * factor,
        )
    }

    pub fn round_out(&self) -> Self {
        let min_x = self.min_x().floor();
        let min_y = self.min_y().floor();
        let max_x = self.max_x().ceil();
        let max_y = self.max_y().ceil();
        Self::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    pub fn subtract_parts(&self, other: &Self) -> RectParts {
        let Some(cut) = self.intersection(other).filter(|_| !self.is_empty()) else {
            return RectParts::whole(*self);
        };
        let mut parts = RectParts::default();
        if cut.min_y() > self.min_y() {
            parts.push(Self::new(
                self.min_x(),
                self.min_y(),
                self.width(),
                cut.min_y() - self.min_y(),
            ));
        }
        if cut.max_y() < self.max_y() {
            parts.push(Self::new(
                self.min_x(),
                cut.max_y(),
                self.width(),
                self.max_y() - cut.max_y(),
            ));
        }
        if cut.min_x() > self.min_x() {
            parts.push(Self::new(
                self.min_x(),
                cut.min_y(),
                cut.min_x() - self.min_x(),
                cut.height(),
            ));
        }
        if cut.max_x() < self.max_x() {
            parts.push(Self::new(
                cut.max_x(),
                cut.min_y(),
                self.max_x() - cut.max_x(),
                cut.height(),
            ));
        }
        parts
    }

    pub fn subtract(&self, other: &Self, into: &mut Vec<Self>) {
        into.extend_from_slice(self.subtract_parts(other).as_slice());
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectParts {
    parts: [Rect; 4],
    len: usize,
}

impl RectParts {
    pub fn whole(rect: Rect) -> Self {
        let mut parts = Self::default();
        if !rect.is_empty() {
            parts.push(rect);
        }
        parts
    }

    pub fn as_slice(&self) -> &[Rect] {
        &self.parts[..self.len]
    }

    fn push(&mut self, rect: Rect) {
        self.parts[self.len] = rect;
        self.len += 1;
    }
}
