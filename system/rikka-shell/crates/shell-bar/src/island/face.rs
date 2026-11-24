use rikka_ui::{Align, Axis, Element, Layout, Length, PaintCtx, Space, View, Widget, motion};
use shell_kit::taskbar::DEPTH;

use crate::axis;

pub(crate) const GAP: Space = Space::TIGHT;

const SHOWN: &str = "island.face";

pub(crate) struct Face {
    children: Vec<Element>,
    shown: bool,
    axis: Axis,
}

pub(crate) fn face(children: Vec<Element>, shown: bool, axis: Axis) -> Face {
    Face {
        children,
        shown,
        axis,
    }
}

impl Widget for Face {
    type State = ();

    fn layout(&self) -> Layout {
        axis::across(axis::layout(self.axis), self.axis, Length::px(DEPTH))
            .gap(GAP)
            .align_items(Align::Center)
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
