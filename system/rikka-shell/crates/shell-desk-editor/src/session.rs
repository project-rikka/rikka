use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use desk_layout::{Guide, Kind, Layout, Projection, Size, WidgetId};
use rikka_types::{Point, Rect};
use rikka_ui::Observers;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shot {
    pub(crate) card: Rect,
    pub(crate) projection: Projection,
    pub(crate) spread: f32,
    pub(crate) moving: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Draft {
    pub(crate) layout: Layout,
    pub(crate) grabbed: Option<WidgetId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Size(Size),
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hovered {
    Widget(WidgetId),
    Tool(Tool),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Offer {
    pub(crate) kind: Kind,
    pub(crate) grab: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DragPreview {
    pub(crate) kind: Kind,
    pub(crate) size: Size,
    pub(crate) at: Point,
    pub(crate) grab: Point,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Marks {
    pub(crate) hovered: Option<Hovered>,
    pub(super) guides: Vec<Guide>,
    pub(super) fading: Option<Instant>,
    pub(super) drag_preview: Option<DragPreview>,
}

#[derive(Clone, Default)]
pub(crate) struct Session(Rc<Inner>);

#[derive(Default)]
struct Inner {
    shot: Cell<Option<Shot>>,
    draft: RefCell<Option<Draft>>,
    drafted: Observers,
    marks: RefCell<Marks>,
    marked: Observers,
    tools: RefCell<Vec<(Tool, Rect)>>,
    offer: Cell<Option<Offer>>,
}

impl Session {
    #[must_use]
    pub(crate) fn shot(&self) -> Option<Shot> {
        self.0.shot.get()
    }

    pub(crate) fn frame(&self, shot: Shot) {
        self.0.shot.set(Some(shot));
    }

    #[must_use]
    pub(crate) fn draft(&self) -> Option<Draft> {
        self.0.drafted.read();
        self.0.draft.borrow().clone()
    }

    pub(super) fn redraft(&self, draft: Option<Draft>) {
        if *self.0.draft.borrow() == draft {
            return;
        }
        *self.0.draft.borrow_mut() = draft;
        self.0.drafted.changed();
    }

    #[must_use]
    pub(crate) fn marks(&self) -> Marks {
        self.0.marked.read();
        self.0.marks.borrow().clone()
    }

    pub(crate) fn mark(&self, change: impl FnOnce(&mut Marks)) {
        let mut marks = self.0.marks.borrow().clone();
        change(&mut marks);
        if *self.0.marks.borrow() == marks {
            return;
        }
        *self.0.marks.borrow_mut() = marks;
        self.0.marked.changed();
    }

    pub(super) fn lay_tools(&self, tools: Vec<(Tool, Rect)>) {
        *self.0.tools.borrow_mut() = tools;
    }

    #[must_use]
    pub(super) fn tool_at(&self, point: Point) -> Option<Tool> {
        self.0
            .tools
            .borrow()
            .iter()
            .find(|(_, rect)| rect.contains(point))
            .map(|(tool, _)| *tool)
    }

    pub(crate) fn offer(&self, offer: Offer) {
        self.0.offer.set(Some(offer));
    }

    #[must_use]
    pub(super) fn take_offer(&self) -> Option<Offer> {
        self.0.offer.take()
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("shot", &self.0.shot.get())
            .finish_non_exhaustive()
    }
}
