use rikka_types::Color;
use rikka_ui::{
    Align, Axis, Element, Inset, Justify, Layout, Length, PaintCtx, Quad, Space, View, Widget,
    motion,
};

use crate::axis;

pub(crate) const HEIGHT: f32 = 28.0;

const GAP: Space = Space::SNUG;

const FAINT: f32 = 0.6;

const BORDER: f32 = 1.0;

const SHAPE: &str = "island.pill";

pub(crate) struct Pill {
    child: Element,
    padding: Inset,
    round: bool,
    axis: Axis,
}

pub(crate) fn pill(child: impl View, padding: Inset, axis: Axis) -> Pill {
    Pill {
        child: child.build(),
        padding,
        round: false,
        axis,
    }
}

pub(super) fn bubble(child: impl View, axis: Axis) -> Pill {
    Pill {
        child: child.build(),
        padding: Inset::NONE,
        round: true,
        axis,
    }
}

impl Widget for Pill {
    type State = ();

    fn layout(&self) -> Layout {
        let layout = axis::across(axis::layout(self.axis), self.axis, Length::px(HEIGHT))
            .align_items(Align::Center)
            .justify(Justify::Center)
            .gap(GAP)
            .padding(self.padding)
            .fixed();
        match self.round {
            true => axis::along_size(layout, self.axis, Length::px(HEIGHT)),
            false => layout,
        }
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let faded = |color: Color| color.with_alpha(color.a * FAINT);
        let shape = ctx.animate_curve(SHAPE, ctx.bounds(), motion::RESHAPE);
        ctx.draw_quad(
            Quad::new(shape)
                .fill(faded(palette.surface))
                .stroke(BORDER, faded(palette.outline))
                .radius(shape.width().min(shape.height()) / 2.0),
        );
    }

    fn debug_name(&self) -> &'static str {
        "IslandPill"
    }
}

impl std::fmt::Debug for Pill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pill")
            .field("round", &self.round)
            .finish_non_exhaustive()
    }
}

impl View for Pill {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}
