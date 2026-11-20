use std::cell::Cell;
use std::rc::Rc;

use rikka_types::{Point, Rect, Size};

#[derive(Clone, Debug, Default)]
pub(crate) struct Selection(Rc<Cell<Option<Drag>>>);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Drag {
    from: Point,
    to: Point,
    settled: bool,
}

impl Selection {
    pub(crate) fn begin(&self, at: Point) {
        self.0.set(Some(Drag {
            from: at,
            to: at,
            settled: false,
        }));
    }

    pub(crate) fn extend(&self, to: Point) {
        if let Some(drag) = self.0.get().filter(|drag| !drag.settled) {
            self.0.set(Some(Drag { to, ..drag }));
        }
    }

    pub(crate) fn settle(&self) {
        if let Some(drag) = self.0.get() {
            self.0.set(Some(Drag {
                settled: true,
                ..drag
            }));
        }
    }

    #[must_use]
    pub(crate) fn rect(&self) -> Option<Rect> {
        self.0.get().map(|drag| {
            let origin = drag.from.min(drag.to);
            let far = drag.from.max(drag.to);
            Rect::from_origin_size(origin, Size::new(far.x - origin.x, far.y - origin.y))
        })
    }

    #[must_use]
    pub(super) fn is_dragging(&self) -> bool {
        self.0.get().is_some_and(|drag| !drag.settled)
    }

    pub(crate) fn clear(&self) {
        self.0.set(None);
    }
}
