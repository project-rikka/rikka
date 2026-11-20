use rikka_compositor_geometry::{Logical, Point, Rect, Size};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Frame {
    pub title: i32,
    pub reach: i32,
}

impl Frame {
    pub const NONE: Self = Self { title: 0, reach: 0 };

    pub fn is_none(self) -> bool {
        self.title == 0 && self.reach == 0
    }

    pub fn offset(self) -> Point<i32, Logical> {
        Point::new(0, self.title)
    }

    pub fn outer_size(self, content: Size<i32, Logical>) -> Size<i32, Logical> {
        Size::new(content.w, content.h + self.title)
    }

    pub fn content_size(self, outer: Size<i32, Logical>) -> Size<i32, Logical> {
        Size::new(outer.w, (outer.h - self.title).max(1))
    }

    pub fn outer(self, content: Rect<i32, Logical>) -> Rect<i32, Logical> {
        Rect::new(content.loc - self.offset(), self.outer_size(content.size))
    }

    pub fn content(self, outer: Rect<i32, Logical>) -> Rect<i32, Logical> {
        Rect::new(outer.loc + self.offset(), self.content_size(outer.size))
    }

    pub fn reached(self, outer: Rect<i32, Logical>) -> Rect<i32, Logical> {
        outer.grown(self.reach)
    }
}
