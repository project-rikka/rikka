use std::fmt;
use std::hash::{Hash, Hasher};

use crate::space::space_name;
use crate::{Buffer, Coordinate, Logical, Physical, Point, Scale, Size, Transform};

pub struct Rect<N, S> {
    pub loc: Point<N, S>,
    pub size: Size<N, S>,
}

impl<N: Coordinate, S> Rect<N, S> {
    pub fn new(loc: Point<N, S>, size: Size<N, S>) -> Self {
        Rect { loc, size }
    }

    pub fn from_size(size: Size<N, S>) -> Self {
        Rect::new(Point::zero(), size)
    }

    pub fn zero() -> Self {
        Rect::new(Point::zero(), Size::zero())
    }

    pub fn from_extremities(
        top_left: impl Into<Point<N, S>>,
        bottom_right: impl Into<Point<N, S>>,
    ) -> Self {
        let top_left = top_left.into();
        Rect::new(top_left, (bottom_right.into() - top_left).to_size())
    }

    pub fn bounding_box(points: impl IntoIterator<Item = Point<N, S>>) -> Self {
        let extremes = points.into_iter().fold(
            None,
            |extremes: Option<(Point<N, S>, Point<N, S>)>, point| {
                Some(match extremes {
                    None => (point, point),
                    Some((min, max)) => (
                        Point::new(point.x.min(min.x), point.y.min(min.y)),
                        Point::new(point.x.max(max.x), point.y.max(max.y)),
                    ),
                })
            },
        );
        match extremes {
            None => Rect::zero(),
            Some((min, max)) => Rect::from_extremities(min, max),
        }
    }

    pub(crate) fn right(&self) -> N {
        self.loc.x.saturating_add(self.size.w)
    }

    pub(crate) fn bottom(&self) -> N {
        self.loc.y.saturating_add(self.size.h)
    }

    pub fn to_f64(self) -> Rect<f64, S> {
        Rect::new(self.loc.to_f64(), self.size.to_f64())
    }

    pub fn upscale(self, scale: impl Into<Scale<N>>) -> Self {
        let scale = scale.into();
        Rect::new(self.loc.upscale(scale), self.size.upscale(scale))
    }

    pub fn downscale(self, scale: impl Into<Scale<N>>) -> Self {
        let scale = scale.into();
        Rect::new(self.loc.downscale(scale), self.size.downscale(scale))
    }

    pub fn is_empty(&self) -> bool {
        self.size.is_empty()
    }

    pub fn contains(self, point: impl Into<Point<N, S>>) -> bool {
        let point = point.into();
        point.x >= self.loc.x
            && point.x < self.right()
            && point.y >= self.loc.y
            && point.y < self.bottom()
    }

    pub fn contains_rect(self, other: impl Into<Rect<N, S>>) -> bool {
        let other = other.into();
        other.loc.x >= self.loc.x
            && other.loc.y >= self.loc.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    pub fn overlaps(self, other: impl Into<Rect<N, S>>) -> bool {
        let other = other.into();
        self.loc.x < other.right()
            && other.loc.x < self.right()
            && self.loc.y < other.bottom()
            && other.loc.y < self.bottom()
    }

    pub fn overlaps_or_touches(self, other: impl Into<Rect<N, S>>) -> bool {
        let other = other.into();
        self.loc.x <= other.right()
            && other.loc.x <= self.right()
            && self.loc.y <= other.bottom()
            && other.loc.y <= self.bottom()
    }

    pub fn intersection(self, other: impl Into<Rect<N, S>>) -> Option<Self> {
        let other = other.into();
        if !self.overlaps(other) {
            return None;
        }
        Some(Rect::from_extremities(
            (self.loc.x.max(other.loc.x), self.loc.y.max(other.loc.y)),
            (
                self.right().min(other.right()),
                self.bottom().min(other.bottom()),
            ),
        ))
    }

    pub fn merge(self, other: Self) -> Self {
        Rect::bounding_box([
            self.loc,
            self.loc + self.size,
            other.loc,
            other.loc + other.size,
        ])
    }

    pub fn grown(self, by: N) -> Self {
        Rect::new(
            self.loc - Point::new(by, by),
            self.size + Size::new(by + by, by + by),
        )
    }

    pub fn subtract_rect(self, other: Self) -> Vec<Self> {
        self.subtract_rects([other])
    }

    pub fn subtract_rects(self, others: impl IntoIterator<Item = Self>) -> Vec<Self> {
        let mut remaining = Vec::with_capacity(4);
        remaining.push(self);
        Rect::subtract_rects_many_in_place(remaining, others)
    }

    pub fn subtract_rects_many_in_place(
        mut rects: Vec<Self>,
        others: impl IntoIterator<Item = Self>,
    ) -> Vec<Self> {
        let mut survivors = Vec::new();
        for other in others {
            if rects.is_empty() {
                break;
            }
            survivors.clear();
            for item in rects.drain(..) {
                match item.intersection(other) {
                    None => survivors.push(item),
                    Some(_) if other.contains_rect(item) => {}
                    Some(hole) => survivors.extend(
                        item.pieces_around(hole)
                            .into_iter()
                            .filter(|piece| !piece.is_empty()),
                    ),
                }
            }
            std::mem::swap(&mut rects, &mut survivors);
        }
        rects
    }

    fn pieces_around(self, hole: Self) -> [Self; 4] {
        let top = Rect::new(
            self.loc,
            (self.size.w, hole.loc.y.saturating_sub(self.loc.y)).into(),
        );
        let left = Rect::new(
            (self.loc.x, hole.loc.y).into(),
            (hole.loc.x.saturating_sub(self.loc.x), hole.size.h).into(),
        );
        let right = Rect::new(
            (hole.right(), hole.loc.y).into(),
            (self.right().saturating_sub(hole.right()), hole.size.h).into(),
        );
        let bottom = Rect::new(
            (self.loc.x, hole.bottom()).into(),
            (self.size.w, self.bottom().saturating_sub(hole.bottom())).into(),
        );
        [top, left, right, bottom]
    }
}

impl<S> Rect<f64, S> {
    pub fn to_i32_round<N: Coordinate>(self) -> Rect<N, S> {
        Rect::new(self.loc.to_i32_round(), self.size.to_i32_round())
    }

    pub fn to_i32_down<N: Coordinate>(self) -> Rect<N, S> {
        Rect::from_extremities(
            self.loc.to_i32_ceil(),
            (self.loc + self.size).to_i32_floor(),
        )
    }

    pub fn to_i32_up<N: Coordinate>(self) -> Rect<N, S> {
        Rect::from_extremities(
            self.loc.to_i32_floor(),
            (self.loc + self.size).to_i32_ceil(),
        )
    }
}

impl<N: Coordinate> Rect<N, Logical> {
    pub fn to_physical(self, scale: impl Into<Scale<N>>) -> Rect<N, Physical> {
        let scale = scale.into();
        Rect::new(self.loc.to_physical(scale), self.size.to_physical(scale))
    }

    pub fn to_physical_precise_round<F: Coordinate, R: Coordinate>(
        self,
        scale: impl Into<Scale<F>>,
    ) -> Rect<R, Physical> {
        self.to_f64()
            .to_physical(scale.into().to_f64())
            .to_i32_round()
    }

    pub fn to_physical_precise_down<F: Coordinate, R: Coordinate>(
        self,
        scale: impl Into<Scale<F>>,
    ) -> Rect<R, Physical> {
        self.to_f64()
            .to_physical(scale.into().to_f64())
            .to_i32_down()
    }

    pub fn to_physical_precise_up<F: Coordinate, R: Coordinate>(
        self,
        scale: impl Into<Scale<F>>,
    ) -> Rect<R, Physical> {
        self.to_f64().to_physical(scale.into().to_f64()).to_i32_up()
    }

    pub fn to_buffer(
        self,
        scale: impl Into<Scale<N>>,
        transform: Transform,
        area: &Size<N, Logical>,
    ) -> Rect<N, Buffer> {
        let rect = self.transformed_in(transform, area);
        let scale = scale.into();
        Rect::new(
            Point::new(rect.loc.x.upscale(scale.x), rect.loc.y.upscale(scale.y)),
            Size::unchecked(rect.size.w.upscale(scale.x), rect.size.h.upscale(scale.y)),
        )
    }
}

impl<N: Coordinate> Rect<N, Physical> {
    pub fn to_logical(self, scale: impl Into<Scale<N>>) -> Rect<N, Logical> {
        let scale = scale.into();
        Rect::new(self.loc.to_logical(scale), self.size.to_logical(scale))
    }
}

impl<N: Coordinate> Rect<N, Buffer> {
    pub fn to_logical(
        self,
        scale: impl Into<Scale<N>>,
        transform: Transform,
        area: &Size<N, Buffer>,
    ) -> Rect<N, Logical> {
        let rect = self.transformed_in(transform.inverse(), area);
        let scale = scale.into();
        Rect::new(
            Point::new(rect.loc.x.downscale(scale.x), rect.loc.y.downscale(scale.y)),
            Size::unchecked(
                rect.size.w.downscale(scale.x),
                rect.size.h.downscale(scale.y),
            ),
        )
    }
}

impl<N: Clone, S> Clone for Rect<N, S> {
    fn clone(&self) -> Self {
        Rect {
            loc: self.loc.clone(),
            size: self.size.clone(),
        }
    }
}

impl<N: Copy, S> Copy for Rect<N, S> {}

impl<N: PartialEq, S> PartialEq for Rect<N, S> {
    fn eq(&self, other: &Self) -> bool {
        self.loc == other.loc && self.size == other.size
    }
}

impl<N: Eq, S> Eq for Rect<N, S> {}

impl<N: Default, S> Default for Rect<N, S> {
    fn default() -> Self {
        Rect {
            loc: Point::default(),
            size: Size::default(),
        }
    }
}

impl<N: fmt::Debug, S> fmt::Debug for Rect<N, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(&format!("Rect<{}>", space_name::<S>()))
            .field("x", &self.loc.x)
            .field("y", &self.loc.y)
            .field("w", &self.size.w)
            .field("h", &self.size.h)
            .finish()
    }
}

impl<N: Hash, S> Hash for Rect<N, S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.loc.hash(state);
        self.size.hash(state);
    }
}
