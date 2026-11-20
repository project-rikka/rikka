use std::cell::Cell;
use std::rc::Rc;

use futures_util::Stream;
use rikka_types::{Point, Rect};
use rikka_worker::Observable;
use shell_kit::Presence;

#[derive(Clone, Debug, Default)]
pub(crate) struct Fold {
    presence: Presence,
    from: Rc<Cell<Point>>,
    moved: Observable<bool>,
}

impl Fold {
    #[must_use]
    pub(crate) fn presence(&self) -> Presence {
        self.presence.clone()
    }

    #[must_use]
    pub(crate) fn from(&self) -> Point {
        self.from.get()
    }

    pub(crate) fn unfold(&self, chevron: Rect) {
        self.from.set(Point::new(chevron.max_x(), chevron.max_y()));
        self.set(true);
    }

    pub(crate) fn fold(&self) {
        self.set(false);
    }

    #[must_use]
    pub(crate) fn is_unfolded(&self) -> bool {
        self.presence.is_shown()
    }

    pub(crate) fn changes(&self) -> impl Stream<Item = bool> + Send + 'static {
        self.moved.changes()
    }

    fn set(&self, up: bool) {
        if self.presence.set(up) {
            self.moved.set(up);
        }
    }
}
