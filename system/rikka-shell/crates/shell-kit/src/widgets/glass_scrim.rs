use rikka_ui::{Element, Layout, Length, PaintCtx, View, Widget, is_exiting};

pub struct GlassScrim {
    child: Element,
    frosted: bool,
}

pub fn glass_scrim(child: impl View) -> GlassScrim {
    GlassScrim {
        child: child.build(),
        frosted: !is_exiting(),
    }
}

impl Widget for GlassScrim {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        if self.frosted {
            ctx.glass(ctx.bounds());
        }
    }

    fn debug_name(&self) -> &'static str {
        "GlassScrim"
    }
}

impl std::fmt::Debug for GlassScrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlassScrim")
            .field("frosted", &self.frosted)
            .finish_non_exhaustive()
    }
}

impl View for GlassScrim {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}
