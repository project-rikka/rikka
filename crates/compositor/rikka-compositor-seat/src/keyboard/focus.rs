use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;

use crate::access::Access;
use crate::carry::{self, Claim, Device};
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
    let left = target_client(state, seat);
    if old.is_some() {
        if !withheld
            && let Some(client) = left
            && state.seats().presents(seat, client, Device::Keyboard)
        {
            conceal(state, seat, client);
        }
        ime::focus_left(state, seat);
    }
    let joined = target.as_ref().map(WlSurface::client);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return false;
    };
    entry.keyboard.focus = focus;
    entry.keyboard.target = target;
    entry.keyboard.entered = focus.filter(|_| !withheld).map(|_| serial);
    if let Some(client) = left
        && joined != Some(client)
    {
        carry::vacate(state, seat, client, Device::Keyboard);
    }
    if let Some(focus) = focus {
        if !withheld && let Some(client) = joined {
            carry::enter(state, seat, client, Device::Keyboard, Claim::Active);
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
    let client = target_client(state, seat);
    if let Some(client) = client
        && state.seats().presents(seat, client, Device::Keyboard)
    {
        conceal(state, seat, client);
    }
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.keyboard.withheld = true;
        entry.keyboard.entered = None;
    }
    if let Some(client) = client {
        carry::vacate(state, seat, client, Device::Keyboard);
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
    if let Some(client) = target_client(state, seat) {
        carry::enter(state, seat, client, Device::Keyboard, Claim::Active);
    }
}

fn withheld<A: Access>(state: &mut A, seat: SeatId) -> bool {
    state
        .seats()
        .get(seat)
        .is_some_and(|entry| entry.keyboard.withheld)
}

fn target_client<A: Access>(state: &mut A, seat: SeatId) -> Option<ClientId> {
    state
        .seats()
        .get(seat)
        .and_then(|entry| entry.keyboard.target.as_ref())
        .map(WlSurface::client)
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
    let client = target_client(state, seat);
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.keyboard.focus = None;
        entry.keyboard.target = None;
        entry.keyboard.entered = None;
    }
    if let Some(client) = client {
        carry::vacate(state, seat, client, Device::Keyboard);
    }
    state.report(SeatEvent::KeyboardFocus { seat, focus: None });
}

pub(crate) fn reveal<A: Access>(state: &mut A, seat: SeatId, client: ClientId) {
    let serial = state.display().next_serial();
    let seats = state.seats();
    let wire = seats.wire(seat, client);
    let Some(entry) = seats.get(seat) else {
        return;
    };
    let board = &entry.keyboard;
    let Some(target) = board
        .target
        .as_ref()
        .filter(|target| !board.withheld && target.client() == client && target.alive())
    else {
        return;
    };
    let pressed = pressed_array(&board.delivered());
    let modifiers = board.sent.modifiers;
    let Some(wired) = seats.get(wire) else {
        return;
    };
    for keyboard in wired.keyboard.keyboards.for_client(client) {
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
    seats.carry_serial(seat, wire, serial);
}

pub(crate) fn conceal<A: Access>(state: &mut A, seat: SeatId, client: ClientId) {
    let serial = state.display().next_serial();
    let seats = state.seats();
    let wire = seats.wire(seat, client);
    let (Some(entry), Some(wired)) = (seats.get(seat), seats.get(wire)) else {
        return;
    };
    let Some(target) = entry
        .keyboard
        .target
        .as_ref()
        .filter(|target| target.client() == client && target.alive())
    else {
        return;
    };
    for keyboard in wired.keyboard.keyboards.for_client(client) {
        keyboard.leave(serial, target);
    }
    seats.carry_serial(seat, wire, serial);
}
