use rikka_types::Point;
use rikka_ui::{Element, Layout, Length, PaintCtx, View, Widget, motion};
use shell_kit::Positioner;

const SHOWN: &str = "ime.shown";

pub(crate) struct Showing {
    child: Element,
    open: bool,
    positioner: Positioner,
}

pub(crate) fn showing(child: impl View, open: bool, positioner: &Positioner) -> Showing {
    Showing {
        child: child.build(),
        open,
        positioner: positioner.clone(),
    }
}

impl Widget for Showing {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::HUG, Length::HUG)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let shown = ctx.progress(SHOWN, self.open, motion::POP, motion::POP_OUT);
        let pivot = match self.positioner.is_flipped() {
            true => Point::new(bounds.x(), bounds.max_y()),
            false => bounds.origin,
        };
        ctx.pop_subtree(shown, pivot);
    }
}

impl View for Showing {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}

impl std::fmt::Debug for Showing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Showing")
            .field("open", &self.open)
            .finish_non_exhaustive()
    }
}
