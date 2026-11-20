use std::time::{Duration, Instant};

use rikka_compositor_geometry::{Logical, Point, Rect, Size};

use crate::render::Visual;
use rikka_compositor_tiling::Edges;

const STRETCH_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Anchor {
    edges: Edges,
    start: Rect<i32, Logical>,
}

impl Anchor {
    pub(crate) fn new(edges: Edges, start: Rect<i32, Logical>) -> Self {
        Self { edges, start }
    }

    pub(crate) fn place(
        &self,
        size: Size<i32, Logical>,
        mut location: Point<i32, Logical>,
    ) -> Point<i32, Logical> {
        if self.edges.contains(Edges::LEFT) {
            location.x = self.start.loc.x + (self.start.size.w - size.w);
        }
        if self.edges.contains(Edges::TOP) {
            location.y = self.start.loc.y + (self.start.size.h - size.h);
        }
        location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stretch {
    pub(crate) rect: Rect<i32, Logical>,
    pub(crate) anchor: Option<Anchor>,
    released_at: Option<Instant>,
}

impl Stretch {
    pub(crate) fn requested(rect: Rect<i32, Logical>, anchor: Option<Anchor>) -> Self {
        Self {
            rect,
            anchor,
            released_at: None,
        }
    }

    pub(crate) fn release(&mut self) {
        self.released_at.get_or_insert_with(Instant::now);
    }

    pub(crate) fn is_released(&self) -> bool {
        self.released_at.is_some()
    }

    pub(crate) fn is_requesting(&self, rect: Rect<i32, Logical>) -> bool {
        self.released_at.is_none() && self.rect == rect
    }

    pub(crate) fn shown(&self, geometry: Rect<i32, Logical>) -> Option<Visual> {
        if self.rect == geometry
            || self
                .released_at
                .is_some_and(|at| at.elapsed() >= STRETCH_TIMEOUT)
        {
            return None;
        }
        Some(Visual::opaque(self.rect))
    }
}

pub(crate) fn gripped(
    from: Rect<i32, Logical>,
    size: Size<i32, Logical>,
    pointer: Point<f64, Logical>,
) -> Point<i32, Logical> {
    let along = |start: i32, at: f64, span: i32, taken: i32| {
        let share = if span > 0 {
            ((at - f64::from(start)) / f64::from(span)).clamp(0.0, 1.0)
        } else {
            0.5
        };
        (at - share * f64::from(taken)).round() as i32
    };
    Point::new(
        along(from.loc.x, pointer.x, from.size.w, size.w),
        along(from.loc.y, pointer.y, from.size.h, size.h),
    )
}
