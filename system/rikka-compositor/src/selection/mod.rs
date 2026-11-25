mod bridge;
mod drag;
mod host;
mod xdnd;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::SeatId;
use rikka_compositor_selection::Selections;
use rikka_compositor_shell::ToplevelId;

pub(crate) use bridge::XBridge;
pub(crate) use xdnd::XDrag;

use crate::render::DragIcon;

#[derive(Debug, Default)]
pub(crate) struct Selection {
    pub(crate) selections: Selections,
    pub(crate) bridge: XBridge,
    pub(crate) xdrag: XDrag,
    icon_at: Option<Point<f64, Logical>>,
}

impl Selection {
    pub(crate) fn icon(&self, seats: [SeatId; 2]) -> Option<DragIcon> {
        seats.into_iter().find_map(|seat| {
            let drag = self.selections.drag(seat)?;
            Some(DragIcon {
                surface: drag.icon?,
                at: drag.at + drag.icon_offset.to_f64(),
            })
        })
    }

    pub(crate) fn drag_drop_target(&self, seat: SeatId) -> Option<ToplevelId> {
        let drag = self.selections.drag(seat)?;
        drag.toplevel.filter(|_| drag.accepted)
    }
}
