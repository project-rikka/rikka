use rikka_settings::session::appearance::TaskbarPosition;
use rikka_ui::{Element, Layout, Length, PaintCtx, State, Styled, View, Widget, is_exiting};
use rikka_widgets::column;
use shell_kit::taskbar::{BORDER, CORNER, Edge};
use shell_kit::{ExclusiveZone, Layer, PointerMode, SurfaceDesc};

use crate::rim::{self, Corners};

#[derive(Debug, State)]
pub(crate) struct Frame {
    edge: Edge,
    corners: Corners,
}

impl Frame {
    fn depth(&self) -> f32 {
        depth(self.corners)
    }
}

#[must_use]
pub(crate) fn surfaces(strip: Edge) -> [SurfaceDesc<Frame>; 3] {
    let [start, end] = strip.sides();
    [
        surface(strip.opposite(), Corners::Rounded),
        surface(start, Corners::Square),
        surface(end, Corners::Square),
    ]
}

fn surface(edge: Edge, corners: Corners) -> SurfaceDesc<Frame> {
    let desc = SurfaceDesc::per_output(move |_| Frame { edge, corners }, view)
        .name(name(edge))
        .layer(Layer::Top)
        .anchor(edge.anchor())
        .exclusive_zone(ExclusiveZone::Fixed(BORDER))
        .pointer(PointerMode::Ignore);
    match edge.is_vertical() {
        true => desc.width(depth(corners)),
        false => desc.height(depth(corners)),
    }
}

fn depth(corners: Corners) -> f32 {
    match corners {
        Corners::Rounded => BORDER + CORNER,
        Corners::Square => BORDER,
    }
}

fn name(edge: Edge) -> &'static str {
    match edge.position() {
        TaskbarPosition::Top => "frame-top",
        TaskbarPosition::Bottom => "frame-bottom",
        TaskbarPosition::Left => "frame-left",
        TaskbarPosition::Right => "frame-right",
    }
}

fn view(frame: &Frame) -> impl View {
    column! {
        if !is_exiting() {
            Element::new(Border {
                edge: frame.edge,
                corners: frame.corners,
            })
            .transition(rim::docking(frame.edge, frame.depth()));
        }
    }
    .fill()
}

struct Border {
    edge: Edge,
    corners: Corners,
}

impl Widget for Border {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        rim::paint(ctx, self.edge, BORDER, self.corners);
    }

    fn debug_name(&self) -> &'static str {
        "TaskbarFrame"
    }
}
