use rikka_ui::{Align, Element, Layout, Length, PaintCtx, Space, Styled, View, Widget, is_exiting};
use rikka_widgets::{column, spacer};
use shell_kit::taskbar::{CORNER, DEPTH, Edge};

use crate::axis;
use crate::island;
use crate::rim::{self, Corners};
use crate::state::Bar;
use crate::strip::parts;

const PADDING: f32 = 12.0;

#[must_use]
pub(crate) fn view(bar: &Bar) -> Element {
    let edge = bar.dock().edge();
    column! {
        if !is_exiting() {
            strip(bar).transition(rim::docking(edge, DEPTH + CORNER));
        }
    }
    .fill()
    .build()
}

fn strip(bar: &Bar) -> Element {
    let dock = bar.dock();
    let axis = dock.axis();
    let row = axis::line(
        axis,
        [
            parts::logo().build(),
            parts::workspaces(bar).build(),
            spacer().grow(1.0).build(),
            parts::cluster(bar).build(),
        ],
    )
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(axis::padding(axis, PADDING, 0.0))
    .build();
    let row = match dock.is_vertical() {
        true => row.width(Length::px(DEPTH)).height(Length::FILL),
        false => row.width(Length::FILL).height(Length::px(DEPTH)),
    };
    Element::with_children(
        Strip { edge: dock.edge() },
        vec![row, island::island(bar.island())],
    )
}

struct Strip {
    edge: Edge,
}

impl Widget for Strip {
    type State = ();

    fn layout(&self) -> Layout {
        axis::layout(self.edge.axis().across())
            .size(Length::FILL, Length::FILL)
            .justify(self.edge.justify())
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        rim::paint(ctx, self.edge, DEPTH, Corners::Rounded);
    }

    fn debug_name(&self) -> &'static str {
        "TaskbarStrip"
    }
}
