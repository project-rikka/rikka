use rikka_ui::{Align, Element, Layout, Length, PaintCtx, Space, View, Widget, motion};
use shell_kit::taskbar::Shape;

pub(crate) const GAP: Space = Space::TIGHT;

const SHOWN: &str = "island.face";

pub(crate) struct Face {
    children: Vec<Element>,
    shown: bool,
}

pub(crate) fn face(children: Vec<Element>, shown: bool) -> Face {
    Face { children, shown }
}

impl Widget for Face {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .gap(GAP)
            .align_items(Align::Center)
            .height(Length::px(Shape.reserved()))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let (target, curve) = match self.shown {
            true => (1.0, motion::FILL),
            false => (0.0, motion::HOVER),
        };
        let shown = ctx.animate_curve(SHOWN, target, curve).clamp(0.0, 1.0);
        ctx.set_subtree_opacity(shown.powi(3));
    }

    fn debug_name(&self) -> &'static str {
        "IslandFace"
    }
}

impl std::fmt::Debug for Face {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Face")
            .field("shown", &self.shown)
            .finish_non_exhaustive()
    }
}

impl View for Face {
    fn build(mut self) -> Element {
        let children = std::mem::take(&mut self.children);
        Element::with_children(self, children)
    }
}
