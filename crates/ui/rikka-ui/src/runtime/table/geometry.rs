use rikka_types::{Point, Rect};

use crate::runtime::layout::{Layouts, Node};
use crate::runtime::table::{CULLED, Table};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Clipping {
    pub(crate) own: Option<Rect>,
    pub(crate) children: Option<Rect>,
    pub(crate) raises: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Reach {
    lo: Point,
    hi: Point,
    hull: Option<Rect>,
}

impl Reach {
    pub(crate) const OPEN: Reach = Reach {
        lo: Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
        hi: Point::new(f32::INFINITY, f32::INFINITY),
        hull: None,
    };

    pub(crate) fn shown(rect: Rect) -> Reach {
        Reach {
            lo: rect.origin,
            hi: Point::new(rect.max_x(), rect.max_y()),
            hull: None,
        }
    }

    pub(crate) fn culled(rect: Rect) -> Reach {
        Reach {
            hull: Some(rect),
            ..Reach::OPEN
        }
    }

    pub(crate) fn join(&mut self, inner: Reach, offset: Point) {
        self.lo = Point::new(
            self.lo.x.max(inner.lo.x + offset.x),
            self.lo.y.max(inner.lo.y + offset.y),
        );
        self.hi = Point::new(
            self.hi.x.min(inner.hi.x + offset.x),
            self.hi.y.min(inner.hi.y + offset.y),
        );
        if let Some(hull) = inner.hull.map(|hull| hull.translate(offset)) {
            self.hull = Some(match self.hull {
                Some(held) => spanned(held, hull),
                None => hull,
            });
        }
    }

    pub(crate) fn holds(&self, seen: Option<Rect>) -> bool {
        let Some(seen) = seen else {
            return self.hull.is_none();
        };
        seen.min_x() <= self.hi.x
            && seen.max_x() >= self.lo.x
            && seen.min_y() <= self.hi.y
            && seen.max_y() >= self.lo.y
            && self.hull.is_none_or(|hull| !hull.touches(&seen))
    }
}

fn spanned(a: Rect, b: Rect) -> Rect {
    let min = Point::new(a.min_x().min(b.min_x()), a.min_y().min(b.min_y()));
    let max = Point::new(a.max_x().max(b.max_x()), a.max_y().max(b.max_y()));
    Rect::new(min.x, min.y, max.x - min.x, max.y - min.y)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Seen {
    pub(crate) origin: Point,
    pub(crate) clip: Option<Rect>,
}

impl Seen {
    pub(crate) const WINDOW: Seen = Seen {
        origin: Point::ZERO,
        clip: None,
    };

    pub(crate) fn bounds(self, table: &Table, node: Node) -> Rect {
        table.placed(node).translate(self.origin)
    }

    pub(crate) fn clip(self, table: &Table, node: Node) -> Option<Rect> {
        let clipping = table.clipping(node);
        if clipping.raises {
            return None;
        }
        narrowed(
            self.clip,
            clipping.own.map(|own| own.translate(self.origin)),
        )
    }

    pub(crate) fn within(self, table: &Table, node: Node) -> Seen {
        let children = table.clipping(node).children;
        Seen {
            origin: self.origin + table.placed(node).origin + table.inner(node),
            clip: narrowed(
                self.clip(table, node),
                children.map(|clip| clip.translate(self.origin)),
            ),
        }
    }

    pub(crate) fn contains(self, table: &Table, node: Node, point: Point) -> bool {
        !table.has(node, CULLED)
            && table
                .get(node)
                .widget
                .hit_test(self.bounds(table, node), point)
            && self
                .clip(table, node)
                .is_none_or(|clip| clip.contains(point))
    }

    pub(crate) fn reaches(self, table: &Table, node: Node, point: Point) -> bool {
        !table.has(node, CULLED) && table.extent(node).translate(self.origin).contains(point)
    }
}

fn narrowed(outer: Option<Rect>, inner: Option<Rect>) -> Option<Rect> {
    match (outer, inner) {
        (outer, None) => outer,
        (None, inner) => inner,
        (Some(outer), Some(inner)) => Some(outer.intersection(&inner).unwrap_or(Rect::ZERO)),
    }
}

impl Table {
    pub(crate) fn seen(&self, layouts: &Layouts, node: Node) -> Seen {
        match layouts.parent(node) {
            Some(parent) => self.seen(layouts, parent).within(self, parent),
            None => Seen::WINDOW,
        }
    }

    pub(crate) fn bounds(&self, layouts: &Layouts, node: Node) -> Rect {
        self.seen(layouts, node).bounds(self, node)
    }

    pub(crate) fn clip(&self, layouts: &Layouts, node: Node) -> Option<Rect> {
        self.seen(layouts, node).clip(self, node)
    }

    pub(crate) fn contains(&self, layouts: &Layouts, node: Node, point: Point) -> bool {
        self.seen(layouts, node).contains(self, node, point)
    }
}
