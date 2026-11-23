mod border;

use rikka_ui::{Element, State, View};
use shell_kit::taskbar::{BORDER, CORNER};
use shell_kit::{Anchor, ExclusiveZone, Layer, PointerMode, Presence, SurfaceDesc};

use crate::frame::border::Border;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Bottom,
    Left,
    Right,
}

impl Side {
    pub(crate) const ARRANGED: [Side; 3] = [Side::Bottom, Side::Left, Side::Right];

    fn name(self) -> &'static str {
        match self {
            Side::Bottom => "frame-bottom",
            Side::Left => "frame-left",
            Side::Right => "frame-right",
        }
    }
}

#[derive(Debug, State)]
pub(crate) struct Frame {
    side: Side,
}

pub(crate) fn surface(side: Side, up: Presence) -> SurfaceDesc<Frame> {
    let desc = SurfaceDesc::per_output(move |_| Frame { side }, view)
        .name(side.name())
        .layer(Layer::Top)
        .exclusive_zone(ExclusiveZone::Fixed(BORDER))
        .pointer(PointerMode::Ignore)
        .presence(up);
    match side {
        Side::Bottom => desc.anchor(Anchor::BOTTOM).height(BORDER + CORNER),
        Side::Left => desc.anchor(Anchor::LEFT).width(BORDER),
        Side::Right => desc.anchor(Anchor::RIGHT).width(BORDER),
    }
}

fn view(frame: &Frame) -> impl View {
    Element::new(Border(frame.side))
}
