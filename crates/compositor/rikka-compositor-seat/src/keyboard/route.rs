use std::time::Duration;

use rikka_compositor_protocols::wl_keyboard::KeyState;
use rikka_compositor_types::Press;

use crate::access::Access;
use crate::event::SeatEvent;
use crate::host::SeatHost;
use crate::ids::{SeatId, VirtualKeyboardId};
use crate::input::{Key, KeySource, millis};
use crate::keyboard::Route;
use crate::keyboard::keystroke::{KeyFilter, Keystroke};
use crate::keyboard::modifiers::SerializedModifiers;
use crate::keyboard::present::{Destination, present, replace_origin_with_devices};
use crate::keyboard::xkb_state::{Origin, XkbState};
use crate::seats::Seats;

pub(crate) fn key<S: SeatHost>(state: &mut S, seat: SeatId, mut key: Key) {
    key.source = classify(state.seats(), seat, key.source);
    let origin = Origin::from_source(key.source);
    let Some(destination) = ready(state.seats(), seat, key.source) else {
        return;
    };
    present(state, seat, origin, destination);
    let Some(keystroke) = read_keystroke(state.seats(), seat, &key) else {
        return;
    };
    let filter = state.filter_key(&keystroke);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry.keyboard.active = origin;
    let Some(xkb_state) = entry.keyboard.xkb_state_mut(origin) else {
        return;
    };
    let route = match key.press {
        Press::Pressed => {
            let route = match (filter, destination) {
                (KeyFilter::Consume, _) => Route::Host,
                (KeyFilter::Forward, Destination::Method) => Route::InputMethod,
                (KeyFilter::Forward, Destination::Clients) => Route::Client,
            };
            xkb_state.hold(key.code, route);
            route
        }
        Press::Released => xkb_state.release(key.code).unwrap_or(Route::Host),
    };
    deliver(state, seat, route, &key);
    present(state, seat, origin, destination);
}

pub(crate) fn virtual_modifiers<A: Access>(
    state: &mut A,
    seat: SeatId,
    keyboard: VirtualKeyboardId,
    modifiers: SerializedModifiers,
) {
    let source = classify(state.seats(), seat, KeySource::Virtual(keyboard));
    let origin = Origin::from_source(source);
    let Some(destination) = ready(state.seats(), seat, source) else {
        return;
    };
    present(state, seat, origin, destination);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry.keyboard.active = origin;
    let Some(xkb_state) = entry.keyboard.xkb_state_mut(origin) else {
        return;
    };
    xkb_state.set_modifiers(modifiers);
    present(state, seat, origin, destination);
    announce_modifiers(state, seat);
}

fn announce_modifiers<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    let held = entry.keyboard.held_modifiers();
    state.report(SeatEvent::Modifiers { seat, held });
}

pub(crate) fn forget_virtual<A: Access>(
    state: &mut A,
    seat: SeatId,
    keyboard: VirtualKeyboardId,
    time: Duration,
) {
    let origin = Origin::Virtual(keyboard);
    let source = classify(state.seats(), seat, KeySource::Virtual(keyboard));
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    let destination = Destination::for_key(entry, source);
    let pressed: Vec<u32> = entry
        .keyboard
        .xkb_state(origin)
        .map(|xkb_state| {
            xkb_state
                .pressed
                .iter()
                .map(|pressed| pressed.code)
                .collect()
        })
        .unwrap_or_default();
    for code in pressed {
        let Some(xkb_state) = state
            .seats()
            .seat_mut(seat)
            .and_then(|entry| entry.keyboard.xkb_state_mut(origin))
        else {
            return;
        };
        xkb_state.update_key(code, Press::Released);
        let Some(route) = xkb_state.release(code) else {
            continue;
        };
        let released = Key {
            code,
            press: Press::Released,
            time,
            source,
        };
        deliver(state, seat, route, &released);
        present(state, seat, origin, destination);
    }
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let keyboard = &mut entry.keyboard;
    keyboard
        .virtuals
        .retain(|xkb_state| xkb_state.origin != origin);
    if keyboard.active == origin {
        keyboard.active = Origin::Devices;
    }
    replace_origin_with_devices(state, seat, origin);
    announce_modifiers(state, seat);
}

fn classify(seats: &mut Seats, seat: SeatId, source: KeySource) -> KeySource {
    let KeySource::Virtual(keyboard) = source else {
        return source;
    };
    let Some(client) = seats
        .virtual_keyboards
        .get(keyboard)
        .map(|board| board.client)
    else {
        return source;
    };
    let composing = seats
        .get(seat)
        .and_then(|entry| entry.input_method_client());
    if composing == Some(client) {
        KeySource::InputMethod(keyboard)
    } else {
        source
    }
}

fn ready(seats: &mut Seats, seat: SeatId, source: KeySource) -> Option<Destination> {
    let keymap = match source {
        KeySource::Device(_) => seats.seat_mut(seat)?.keyboard.default.clone(),
        KeySource::Virtual(board) | KeySource::InputMethod(board) => {
            seats.virtual_keyboards.get(board)?.keymap.clone()?
        }
    };
    let entry = seats.seat_mut(seat)?;
    let destination = Destination::for_key(entry, source);
    let origin = Origin::from_source(source);
    let keyboard = &mut entry.keyboard;
    match keyboard.xkb_state_mut(origin) {
        Some(xkb_state) => xkb_state.rekey(&keymap),
        None => keyboard.virtuals.push(XkbState::new(origin, keymap)),
    }
    Some(destination)
}

fn read_keystroke(seats: &mut Seats, seat: SeatId, key: &Key) -> Option<Keystroke> {
    let entry = seats.seat_mut(seat)?;
    let inhibited = entry.keyboard.shortcuts_inhibited();
    let xkb_state = entry
        .keyboard
        .xkb_state_mut(Origin::from_source(key.source))?;
    xkb_state.update_key(key.code, key.press);
    Some(Keystroke::read(
        seat,
        key,
        xkb_state.modifiers(),
        &xkb_state.state,
        xkb_state.keymap.xkb(),
        inhibited,
    ))
}

fn deliver<A: Access>(state: &mut A, seat: SeatId, route: Route, key: &Key) {
    match route {
        Route::Host => {}
        Route::InputMethod => {
            let serial = state.display().next_serial();
            if let Some(grab) = state
                .seats()
                .get(seat)
                .and_then(|entry| entry.ime.grab.as_ref())
            {
                grab.resource
                    .send_key(serial, millis(key.time), key.code, key_state(key.press));
            }
        }
        Route::Client => {
            let Some(client) = state
                .seats()
                .get(seat)
                .and_then(|entry| entry.keyboard.focused_client())
            else {
                return;
            };
            let serial = state.display().next_serial();
            let Some(entry) = state.seats().get(seat) else {
                return;
            };
            for keyboard in entry.keyboard.keyboards.for_client(client) {
                keyboard.send_key(serial, millis(key.time), key.code, key_state(key.press));
            }
        }
    }
}

fn key_state(press: Press) -> KeyState {
    match press {
        Press::Pressed => KeyState::Pressed,
        Press::Released => KeyState::Released,
    }
}
