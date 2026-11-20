use std::cell::Cell;
use std::rc::Rc;

use rikka_types::Point;

#[derive(Clone, Debug, Default)]
pub struct Spot(Rc<Cell<Option<Point>>>);

impl Spot {
    #[must_use]
    pub(crate) fn at(&self) -> Option<Point> {
        self.0.get()
    }

    pub(crate) fn put(&self, at: Point) {
        self.0.set(Some(at));
    }
}
