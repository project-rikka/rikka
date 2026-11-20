use rikka_compositor_protocols::zwp_keyboard_shortcuts_inhibit_manager_v1::{
    self as manager, ZwpKeyboardShortcutsInhibitManagerV1,
};
use rikka_compositor_protocols::zwp_keyboard_shortcuts_inhibitor_v1::{
    self as inhibitor_protocol, ZwpKeyboardShortcutsInhibitorV1,
};
use rikka_compositor_surface::SurfaceId;

use crate::access::Access;
use crate::ids::SeatId;
use crate::surface;

#[derive(Debug)]
pub(crate) struct Inhibitor {
    pub(crate) resource: ZwpKeyboardShortcutsInhibitorV1,
    pub(crate) surface: SurfaceId,
}

pub(crate) fn inhibit_manager<A: Access>(
    state: &mut A,
    manager: ZwpKeyboardShortcutsInhibitManagerV1,
    request: manager::Request,
) {
    let manager::Request::InhibitShortcuts { id, surface, seat } = request else {
        return;
    };
    let Some(seat) = seat.key::<SeatId>() else {
        return;
    };
    let Some(surface) = surface::id(state, &surface) else {
        return;
    };
    id.set_key(seat);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let keyboard = &mut entry.keyboard;
    if keyboard
        .inhibitors
        .iter()
        .any(|inhibitor| inhibitor.surface == surface && inhibitor.resource.alive())
    {
        manager.post_error(manager::Error::AlreadyInhibited);
        return;
    }
    id.active();
    keyboard.inhibitors.push(Inhibitor {
        resource: id,
        surface,
    });
}

pub(crate) fn inhibitor<A: Access>(
    _state: &mut A,
    _inhibitor: ZwpKeyboardShortcutsInhibitorV1,
    _request: inhibitor_protocol::Request,
) {
}

pub(crate) fn inhibitor_destroyed<A: Access>(
    state: &mut A,
    inhibitor: ZwpKeyboardShortcutsInhibitorV1,
) {
    let Some(seat) = inhibitor.key::<SeatId>() else {
        return;
    };
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry
            .keyboard
            .inhibitors
            .retain(|shortcuts_inhibitor| shortcuts_inhibitor.resource != inhibitor);
    }
}
