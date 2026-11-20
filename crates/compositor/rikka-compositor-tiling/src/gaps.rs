use rikka_compositor_geometry::{Logical, Point, Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gaps {
    pub inner: i32,
    pub outer: i32,
}

impl Gaps {
    pub fn partitioned(self, area: Rect<i32, Logical>) -> Rect<i32, Logical> {
        inset(area, (self.outer - self.inner / 2).max(0))
    }

    pub fn inset(self, tile: Rect<i32, Logical>) -> Rect<i32, Logical> {
        inset(tile, self.inner / 2)
    }
}

fn inset(rect: Rect<i32, Logical>, by: i32) -> Rect<i32, Logical> {
    let by = by
        .clamp(0, (rect.size.w - 1).max(0) / 2)
        .min((rect.size.h - 1).max(0) / 2);
    Rect::new(
        rect.loc + Point::new(by, by),
        Size::new((rect.size.w - 2 * by).max(1), (rect.size.h - 2 * by).max(1)),
    )
}
