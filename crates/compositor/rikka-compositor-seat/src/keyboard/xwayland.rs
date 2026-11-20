use rikka_compositor_protocols::zwp_xwayland_keyboard_grab_manager_v1::{
    self as manager, ZwpXwaylandKeyboardGrabManagerV1,
};
use rikka_compositor_protocols::zwp_xwayland_keyboard_grab_v1::{
    self as grab, ZwpXwaylandKeyboardGrabV1,
};
use rikka_compositor_surface::SurfaceId;

use crate::access::Access;
use crate::event::SeatEvent;
use crate::focus::KeyboardFocus;
use crate::ids::SeatId;
use crate::keyboard::KeyboardGrab;
use crate::keyboard::focus::set_focus;
use crate::surface;

#[derive(Debug)]
pub(crate) struct XwaylandGrab {
    pub(crate) resource: ZwpXwaylandKeyboardGrabV1,
    pub(crate) surface: SurfaceId,
}

pub(crate) fn xwayland_manager<A: Access>(
    state: &mut A,
    _manager: ZwpXwaylandKeyboardGrabManagerV1,
    request: manager::Request,
) {
    let manager::Request::GrabKeyboard { id, surface, seat } = request else {
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
    entry.keyboard.xwayland = Some(XwaylandGrab {
        resource: id,
        surface,
    });
    let keeps = entry
        .keyboard
        .focus
        .is_some_and(|focus| focus.surface() == surface);
    if !keeps {
        set_focus(state, seat, Some(KeyboardFocus::Surface(surface)), true);
    }
    state.report(SeatEvent::KeyboardGrab {
        seat,
        grab: KeyboardGrab::Xwayland { surface },
        active: true,
    });
}

pub(crate) fn xwayland_grab<A: Access>(
    _state: &mut A,
    _grab: ZwpXwaylandKeyboardGrabV1,
    _request: grab::Request,
) {
}

pub(crate) fn xwayland_grab_destroyed<A: Access>(state: &mut A, grab: ZwpXwaylandKeyboardGrabV1) {
    let Some(seat) = grab.key::<SeatId>() else {
        return;
    };
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(surface) = entry
        .keyboard
        .xwayland
        .as_ref()
        .filter(|xwayland_grab| xwayland_grab.resource == grab)
        .map(|xwayland_grab| xwayland_grab.surface)
    else {
        return;
    };
    entry.keyboard.xwayland = None;
    state.report(SeatEvent::KeyboardGrab {
        seat,
        grab: KeyboardGrab::Xwayland { surface },
        active: false,
    });
}
