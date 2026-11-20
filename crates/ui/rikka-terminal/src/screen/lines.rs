use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rikka_types::{Point, Rect, Size};
use rikka_ui::{Layout, Length, NodeHandle, PaintCtx, Widget};

#[derive(Default)]
pub(crate) struct LinesCell {
    top: Cell<i32>,
    painter: RefCell<Option<NodeHandle>>,
}

impl LinesCell {
    pub(crate) fn top(&self) -> i32 {
        self.top.get()
    }

    pub(crate) fn scroll_to(&self, top: i32) {
        if self.top.replace(top) == top {
            return;
        }
        if let Some(painter) = self.painter.borrow().as_ref() {
            painter.repaint();
        }
    }
}

impl std::fmt::Debug for LinesCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinesCell")
            .field("top", &self.top.get())
            .finish_non_exhaustive()
    }
}

pub(crate) struct Lines {
    shown: Rc<LinesCell>,
    height: f32,
}

impl Lines {
    pub(crate) fn new(shown: &Rc<LinesCell>, height: f32) -> Lines {
        Lines {
            shown: Rc::clone(shown),
            height,
        }
    }
}

impl Widget for Lines {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::FILL)
            .placed_at(Point::ZERO)
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.shown, &other.shown) && self.height == other.height
    }

    fn children_offset(&self, _state: &(), _bounds: Rect, _content: Size) -> Point {
        Point::new(0.0, -(self.shown.top.get() as f32) * self.height)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        *self.shown.painter.borrow_mut() = Some(ctx.handle());
    }
}
