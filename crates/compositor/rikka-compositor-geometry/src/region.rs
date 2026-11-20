use std::fmt;
use std::marker::PhantomData;

use crate::bands::{Band, Bands, Edges, Op, Span};
use crate::{Point, Rect, Size};

pub struct Region<S> {
    bands: Bands,
    space: PhantomData<S>,
}

impl<S> Region<S> {
    pub const fn new() -> Self {
        Region {
            bands: Bands::new(),
            space: PhantomData,
        }
    }

    pub fn add(&mut self, rect: Rect<i32, S>) {
        let edges = edges(rect);
        if edges.is_empty() {
            return;
        }
        if self.bands.below_all(edges) {
            self.bands.append(edges);
            return;
        }
        if self.bands.covers(edges) {
            return;
        }
        self.combine(&Bands::single(edges), Op::Union);
    }

    pub fn subtract(&mut self, rect: Rect<i32, S>) {
        let edges = edges(rect);
        if edges.is_empty() || self.bands.is_empty() {
            return;
        }
        self.combine(&Bands::single(edges), Op::Subtract);
    }

    pub fn subtract_region(&mut self, other: &Region<S>) {
        if other.bands.is_empty() || self.bands.is_empty() {
            return;
        }
        self.combine(&other.bands, Op::Subtract);
    }

    pub fn union(&mut self, other: &Region<S>) {
        if other.bands.is_empty() {
            return;
        }
        if self.bands.is_empty() {
            self.bands.clone_from(&other.bands);
            return;
        }
        self.combine(&other.bands, Op::Union);
    }

    pub fn intersect(&self, area: Rect<i32, S>) -> Self {
        let mut clipped = Region::new();
        Bands::combine(
            &self.bands,
            &Bands::single(edges(area)),
            Op::Intersect,
            &mut clipped.bands,
        );
        clipped
    }

    pub fn contains(&self, point: impl Into<Point<i32, S>>) -> bool {
        let point = point.into();
        self.bands.contains(point.x, point.y)
    }

    pub fn span_at(&self, point: impl Into<Point<i32, S>>) -> Option<Rect<i32, S>> {
        let point = point.into();
        self.bands.span_containing(point.x, point.y).map(rect)
    }

    pub fn covers(&self, rect: Rect<i32, S>) -> bool {
        self.bands.covers(edges(rect))
    }

    pub fn overlaps(&self, rect: Rect<i32, S>) -> bool {
        self.bands.overlaps(edges(rect))
    }

    pub fn clear(&mut self) {
        self.bands.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.bands.is_empty()
    }

    pub fn bounding_box(&self) -> Rect<i32, S> {
        self.bands.bounds().map_or_else(Rect::zero, rect)
    }

    pub fn translate(&mut self, offset: Point<i32, S>) {
        self.bands.translate(offset.x, offset.y);
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = Rect<i32, S>> + '_ {
        Rects {
            bands: &self.bands.bands,
            spans: &self.bands.spans,
            band: 0,
            span: 0,
            space: PhantomData,
        }
    }

    fn combine(&mut self, other: &Bands, op: Op) {
        let mut combined = Bands::default();
        Bands::combine(&self.bands, other, op, &mut combined);
        self.bands = combined;
    }
}

struct Rects<'a, S> {
    bands: &'a [Band],
    spans: &'a [Span],
    band: usize,
    span: usize,
    space: PhantomData<S>,
}

impl<S> Iterator for Rects<'_, S> {
    type Item = Rect<i32, S>;

    fn next(&mut self) -> Option<Rect<i32, S>> {
        let span = self.spans.get(self.span)?;
        while self.bands[self.band].end as usize <= self.span {
            self.band += 1;
        }
        let band = self.bands[self.band];
        self.span += 1;
        Some(rect(Edges {
            left: span.left,
            top: band.top,
            right: span.right,
            bottom: band.bottom,
        }))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.spans.len() - self.span;
        (left, Some(left))
    }
}

impl<S> ExactSizeIterator for Rects<'_, S> {}

fn edges<S>(rect: Rect<i32, S>) -> Edges {
    Edges {
        left: rect.loc.x,
        top: rect.loc.y,
        right: rect.right(),
        bottom: rect.bottom(),
    }
}

fn rect<S>(edges: Edges) -> Rect<i32, S> {
    Rect::new(
        Point::new(edges.left, edges.top),
        Size::new(
            edges.right.saturating_sub(edges.left),
            edges.bottom.saturating_sub(edges.top),
        ),
    )
}

impl<S> From<Rect<i32, S>> for Region<S> {
    fn from(rect: Rect<i32, S>) -> Self {
        Region {
            bands: Bands::single(edges(rect)),
            space: PhantomData,
        }
    }
}

impl<S> FromIterator<Rect<i32, S>> for Region<S> {
    fn from_iter<I: IntoIterator<Item = Rect<i32, S>>>(rects: I) -> Self {
        let mut region = Region::new();
        for rect in rects {
            region.add(rect);
        }
        region
    }
}

impl<S> Default for Region<S> {
    fn default() -> Self {
        Region::new()
    }
}

impl<S> Clone for Region<S> {
    fn clone(&self) -> Self {
        Region {
            bands: self.bands.clone(),
            space: PhantomData,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.bands.clone_from(&source.bands);
    }
}

impl<S> PartialEq for Region<S> {
    fn eq(&self, other: &Self) -> bool {
        self.bands == other.bands
    }
}

impl<S> Eq for Region<S> {}

impl<S> fmt::Debug for Region<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
