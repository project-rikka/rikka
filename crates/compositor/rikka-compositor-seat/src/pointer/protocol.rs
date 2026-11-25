use rikka_compositor_geometry::Point;
use rikka_compositor_protocols::wl_pointer::{self, Request, WlPointer};

use crate::access::Access;
use crate::carry::Device;
use crate::host::SeatHost;
use crate::ids::SeatId;
use crate::pointer::cursor::may_set_cursor;
use crate::pointer::cursor::{CursorOwner, set_cursor};

pub(crate) fn pointer_request<S: SeatHost>(state: &mut S, pointer: WlPointer, request: Request) {
    let Request::SetCursor {
        serial,
        surface,
        hotspot_x,
        hotspot_y,
    } = request
    else {
        return;
    };
    let Some(wire) = pointer.key::<SeatId>() else {
        return;
    };
    let client = pointer.client();
    let seat = state.seats().presenter(wire, client, Device::Pointer);
    if !may_set_cursor(state, seat, CursorOwner::Pointer, client, serial) {
        return;
    }
    if let Err(error) = set_cursor(
        state,
        seat,
        CursorOwner::Pointer,
        surface,
        Point::new(hotspot_x, hotspot_y),
    ) {
        error.post(&pointer, wl_pointer::Error::Role.code());
    }
}

pub(crate) fn pointer_destroyed<A: Access>(state: &mut A, pointer: WlPointer) {
    let Some(seat) = pointer.key::<SeatId>() else {
        return;
    };
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.pointer.pointers.remove(&pointer);
    }
}
