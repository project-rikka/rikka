use rikka_compositor_surface::SurfaceId;

use crate::access::Access;
use crate::event::SeatEvent;
use crate::focus::KeyboardFocus;
use crate::ids::SeatId;
use crate::ime;
use crate::keyboard::KeyboardGrab;
use crate::keyboard::deliver::{pressed_array, send_keymap};
use crate::surface;

pub(crate) fn set_focus<A: Access>(
    state: &mut A,
    seat: SeatId,
    focus: Option<KeyboardFocus>,
    forced: bool,
) -> bool {
    let Some(entry) = state.seats().get(seat) else {
        return false;
    };
    let old = entry.keyboard.focus;
    let pinned = entry.keyboard.xwayland.as_ref().map(|grab| grab.surface);
    if !forced
        && let Some(pinned) = pinned
        && focus.map(|focus| focus.surface()) != Some(pinned)
    {
        return false;
    }
    if old == focus {
        return true;
    }
    let target = focus.and_then(|focus| surface::resource(state, focus.surface()));
    let focus = focus.filter(|_| target.is_some());
    let same_surface = old.map(|old| old.surface()) == focus.map(|focus| focus.surface());
    if same_surface {
        if let Some(entry) = state.seats().seat_mut(seat) {
            entry.keyboard.focus = focus;
        }
        state.report(SeatEvent::KeyboardFocus { seat, focus });
        return true;
    }
    let serial = state.display().next_serial();
    let withheld = withheld(state, seat);
    if old.is_some() {
        if !withheld {
            leave(state, seat, serial);
        }
        ime::focus_left(state, seat);
    }
    let Some(entry) = state.seats().seat_mut(seat) else {
        return false;
    };
    entry.keyboard.focus = focus;
    entry.keyboard.target = target;
    entry.keyboard.entered = focus.filter(|_| !withheld).map(|_| serial);
    if let Some(focus) = focus {
        if !withheld {
            enter(state, seat, serial);
        }
        ime::focus_entered(state, seat, focus.surface());
    }
    state.report(SeatEvent::KeyboardFocus { seat, focus });
    true
}

pub(crate) fn withhold<A: Access>(state: &mut A, seat: SeatId) {
    if withheld(state, seat) {
        return;
    }
    let serial = state.display().next_serial();
    leave(state, seat, serial);
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.keyboard.withheld = true;
        entry.keyboard.entered = None;
    }
}

pub(crate) fn restore<A: Access>(state: &mut A, seat: SeatId) {
    if !withheld(state, seat) {
        return;
    }
    let serial = state.display().next_serial();
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry.keyboard.withheld = false;
    entry.keyboard.entered = entry.keyboard.focus.map(|_| serial);
    enter(state, seat, serial);
}

fn withheld<A: Access>(state: &mut A, seat: SeatId) -> bool {
    state
        .seats()
        .get(seat)
        .is_some_and(|entry| entry.keyboard.withheld)
}

pub(crate) fn refocus_after_destroy<A: Access>(state: &mut A, seat: SeatId, surface: SurfaceId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry
        .keyboard
        .inhibitors
        .retain(|inhibitor| inhibitor.surface != surface);
    if entry
        .keyboard
        .xwayland
        .as_ref()
        .is_some_and(|grab| grab.surface == surface)
    {
        entry.keyboard.xwayland = None;
        state.report(SeatEvent::KeyboardGrab {
            seat,
            grab: KeyboardGrab::Xwayland { surface },
            active: false,
        });
    }
    let focused = state
        .seats()
        .get(seat)
        .and_then(|entry| entry.keyboard.focus)
        .is_some_and(|focus| focus.surface() == surface);
    if !focused {
        return;
    }
    ime::focus_left(state, seat);
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.keyboard.focus = None;
        entry.keyboard.target = None;
        entry.keyboard.entered = None;
    }
    state.report(SeatEvent::KeyboardFocus { seat, focus: None });
}

fn leave<A: Access>(state: &mut A, seat: SeatId, serial: u32) {
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    let Some(target) = entry
        .keyboard
        .target
        .as_ref()
        .filter(|target| target.alive())
    else {
        return;
    };
    for keyboard in entry.keyboard.keyboards.for_client(target.client()) {
        keyboard.leave(serial, target);
    }
}

fn enter<A: Access>(state: &mut A, seat: SeatId, serial: u32) {
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    let board = &entry.keyboard;
    let Some(target) = &board.target else {
        return;
    };
    let pressed = pressed_array(&board.delivered());
    let modifiers = board.sent.modifiers;
    for keyboard in board.keyboards.for_client(target.client()) {
        send_keymap(keyboard, &board.sent.keymap);
        keyboard.enter(serial, target, &pressed);
        keyboard.modifiers(
            serial,
            modifiers.depressed,
            modifiers.latched,
            modifiers.locked,
            modifiers.group,
        );
    }
}
