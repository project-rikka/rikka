use rikka_compositor_protocols::wl_keyboard::{Request, WlKeyboard};

use crate::access::Access;
use crate::ids::SeatId;

pub(crate) fn keyboard_request<A: Access>(
    _state: &mut A,
    _keyboard: WlKeyboard,
    _request: Request,
) {
}

pub(crate) fn keyboard_destroyed<A: Access>(state: &mut A, keyboard: WlKeyboard) {
    let Some(seat) = keyboard.key::<SeatId>() else {
        return;
    };
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.keyboard.keyboards.remove(&keyboard);
    }
}
