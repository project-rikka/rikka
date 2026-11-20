use rikka_ui::{Element, Layout, PaintCtx, View, Widget};

pub struct Clip {
    child: Element,
}

pub fn clip(child: impl View) -> Clip {
    Clip {
        child: child.build(),
    }
}

impl Widget for Clip {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().grow(1.0)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        ctx.set_children_clip(ctx.bounds());
    }

    fn debug_name(&self) -> &'static str {
        "Clip"
    }
}

impl std::fmt::Debug for Clip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Clip").finish_non_exhaustive()
    }
}

impl View for Clip {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}
