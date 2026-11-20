use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use rikka_ui::{Element, Layout, Length, PaintCtx, Widget};

#[derive(Debug, Default)]
pub(crate) struct Seen {
    pub(crate) painted: RefCell<BTreeMap<usize, f32>>,
}

impl Seen {
    pub(crate) fn at(&self, id: usize) -> Option<f32> {
        self.painted.borrow().get(&id).copied()
    }
}

struct Marker {
    id: usize,
    seen: Rc<Seen>,
    height: f32,
}

impl Widget for Marker {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(self.height))
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        self.seen
            .painted
            .borrow_mut()
            .insert(self.id, ctx.bounds().y());
    }
}

pub(crate) fn marker(id: usize, seen: &Rc<Seen>, height: f32) -> Element {
    Element::new(Marker {
        id,
        seen: Rc::clone(seen),
        height,
    })
}
