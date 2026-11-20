use rikka_compositor_protocols::zwp_relative_pointer_manager_v1::{
    self as manager, ZwpRelativePointerManagerV1,
};
use rikka_compositor_protocols::zwp_relative_pointer_v1::{self as relative, ZwpRelativePointerV1};

use crate::access::Access;
use crate::ids::SeatId;

pub(crate) fn relative_manager<A: Access>(
    state: &mut A,
    _manager: ZwpRelativePointerManagerV1,
    request: manager::Request,
) {
    let manager::Request::GetRelativePointer { id, pointer } = request else {
        return;
    };
    let Some(seat) = pointer.key::<SeatId>() else {
        return;
    };
    id.set_key(seat);
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.pointer.relative.add(id);
    }
}

pub(crate) fn relative_request<A: Access>(
    _state: &mut A,
    _relative: ZwpRelativePointerV1,
    _request: relative::Request,
) {
}

pub(crate) fn relative_destroyed<A: Access>(state: &mut A, relative: ZwpRelativePointerV1) {
    let Some(seat) = relative.key::<SeatId>() else {
        return;
    };
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.pointer.relative.remove(&relative);
    }
}
