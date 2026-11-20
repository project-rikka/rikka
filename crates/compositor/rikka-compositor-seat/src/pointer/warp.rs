use rikka_compositor_geometry::Point;
use rikka_compositor_protocols::wp_pointer_warp_v1::{Request, WpPointerWarpV1};

use crate::access::Access;
use crate::event::SeatEvent;
use crate::ids::SeatId;
use crate::pointer::cursor::CursorOwner;
use crate::pointer::cursor::may_set_cursor;
use crate::surface;

pub(crate) fn warp_request<A: Access>(state: &mut A, warp: WpPointerWarpV1, request: Request) {
    let Request::WarpPointer {
        surface,
        pointer,
        x,
        y,
        serial,
    } = request
    else {
        return;
    };
    let Some(seat) = pointer.key::<SeatId>() else {
        return;
    };
    let Some(surface) = surface::id(state, &surface) else {
        return;
    };
    if !may_set_cursor(state, seat, CursorOwner::Pointer, warp.client(), serial) {
        return;
    }
    let Some(focus) = state
        .seats()
        .get(seat)
        .and_then(|entry| entry.pointer.focus())
        .filter(|focus| focus.surface == surface)
    else {
        return;
    };
    state.report(SeatEvent::Warp {
        seat,
        location: focus.origin + Point::new(x, y),
    });
}
