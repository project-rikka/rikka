use std::cell::Cell;
use std::rc::Rc;

use futures_util::Stream;
use rikka_worker::Observable;

#[derive(Clone, Debug, Default)]
pub(crate) struct Percentage {
    shown: Rc<Cell<bool>>,
    changed: Observable<()>,
}

impl Percentage {
    pub(crate) fn set(&self, shown: bool) {
        if self.shown.replace(shown) != shown {
            self.changed.set(());
        }
    }

    #[must_use]
    pub(crate) fn is_shown(&self) -> bool {
        self.shown.get()
    }

    pub(crate) fn changes(&self) -> impl Stream<Item = ()> + Send + 'static {
        self.changed.changes()
    }
}
