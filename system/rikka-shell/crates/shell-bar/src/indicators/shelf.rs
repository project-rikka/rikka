use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures_util::{Stream, StreamExt};
use rikka_seam::SeamClient;
use rikka_types::{Point, Rect};
use rikka_ui::Element;
use rikka_worker::Observable;
use shell_kit::Presence;
use shell_kit::launch::Spawn;

use crate::indicators::{Indicator, Kind};

#[derive(Clone)]
pub(crate) struct Shelf(Rc<Inner>);

struct Inner {
    seam: SeamClient,
    all: Vec<Rc<dyn Indicator>>,
    shown: RefCell<Vec<Kind>>,
    open: Cell<Option<Kind>>,
    from: Cell<Point>,
    presence: Presence,
    changed: Observable<()>,
}

impl Shelf {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, all: Vec<Rc<dyn Indicator>>) -> Shelf {
        let shown = all.iter().map(|indicator| indicator.kind()).collect();
        Shelf(Rc::new(Inner {
            seam,
            all,
            shown: RefCell::new(shown),
            open: Cell::new(None),
            from: Cell::new(Point::new(0.0, 0.0)),
            presence: Presence::default(),
            changed: Observable::default(),
        }))
    }

    pub(crate) fn show(&self, kinds: &[String]) {
        let shown = Kind::parse_list(kinds);
        if self.0.open.get().is_some_and(|open| !shown.contains(&open)) {
            self.close();
        }
        self.0.shown.replace(shown);
        self.0.changed.set(());
    }

    #[must_use]
    pub(crate) fn marks(&self) -> Vec<(Kind, Element)> {
        self.shown()
            .filter_map(|indicator| Some((indicator.kind(), indicator.mark()?)))
            .collect()
    }

    pub(crate) fn open(&self, kind: Kind, at: Rect) {
        let Some(indicator) = self.shown().find(|indicator| indicator.kind() == kind) else {
            return;
        };
        self.0.from.set(Point::new(at.max_x(), at.max_y()));
        self.0.open.set(Some(kind));
        self.0.presence.set(true);
        indicator.on_open();
        self.0.changed.set(());
    }

    pub(crate) fn close(&self) {
        if self.0.presence.set(false) {
            self.0.changed.set(());
        }
    }

    #[must_use]
    pub(crate) fn open_indicator(&self) -> Option<Rc<dyn Indicator>> {
        let open = self.0.open.get()?;
        self.shown().find(|indicator| indicator.kind() == open)
    }

    #[must_use]
    pub(crate) fn is_open(&self, kind: Kind) -> bool {
        self.0.presence.is_shown() && self.0.open.get() == Some(kind)
    }

    #[must_use]
    pub(crate) fn from(&self) -> Point {
        self.0.from.get()
    }

    #[must_use]
    pub(crate) fn presence(&self) -> Presence {
        self.0.presence.clone()
    }

    pub(crate) fn tick(&self) {
        for indicator in self.shown() {
            indicator.tick();
        }
    }

    pub(crate) fn launch<S: Into<String>>(&self, argv: impl IntoIterator<Item = S>) {
        self.0.seam.notify(Spawn::new(argv).request());
        self.close();
    }

    pub(crate) fn changes(&self) -> impl Stream<Item = ()> + Send + 'static {
        futures_util::stream::select_all(
            self.0
                .all
                .iter()
                .map(|indicator| indicator.changes())
                .chain([self.0.changed.changes().boxed()]),
        )
    }

    fn shown(&self) -> impl Iterator<Item = Rc<dyn Indicator>> + '_ {
        let shown = self.0.shown.borrow().clone();
        self.0
            .all
            .iter()
            .filter(move |indicator| shown.contains(&indicator.kind()))
            .cloned()
    }
}

impl std::fmt::Debug for Shelf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shelf")
            .field("shown", &self.0.shown.borrow())
            .field("open", &self.0.open.get())
            .finish_non_exhaustive()
    }
}
