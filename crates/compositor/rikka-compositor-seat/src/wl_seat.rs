use rikka_compositor_protocols::wl_seat::{Request, WlSeat};

use crate::access::Access;
use crate::ids::SeatId;
use crate::{keyboard, pointer};

pub(crate) fn seat_bound<A: Access>(state: &mut A, handle: WlSeat) {
    let Some(seat) = handle.key::<SeatId>() else {
        return;
    };
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    handle.capabilities(entry.capabilities.wire());
    handle.name(&entry.name);
    entry.handles.push(handle);
}

pub(crate) fn seat_request<A: Access>(state: &mut A, handle: WlSeat, request: Request) {
    let Some(seat) = handle.key::<SeatId>() else {
        return;
    };
    match request {
        Request::GetPointer { id } => {
            id.set_key(seat);
            pointer::introduce(state, seat, id);
        }
        Request::GetKeyboard { id } => {
            id.set_key(seat);
            keyboard::introduce(state, seat, id);
        }
        Request::GetTouch { id } => {
            id.set_key(seat);
            if let Some(entry) = state.seats().seat_mut(seat) {
                entry.touch.touches.add(id);
            }
        }
        Request::Release => {}
    }
}

pub(crate) fn seat_destroyed<A: Access>(state: &mut A, handle: WlSeat) {
    let Some(seat) = handle.key::<SeatId>() else {
        return;
    };
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.handles.retain(|bound| *bound != handle);
    }
}
