use rikka_compositor_protocols::wl_keyboard::KeymapFormat;

use crate::access::Access;
use crate::carry::Device;
use crate::ids::SeatId;
use crate::input::KeySource;
use crate::keyboard::Repeat;
use crate::keyboard::deliver::{send_default_out_of_focus, send_keymap};
use crate::keyboard::keymap::Keymap;
use crate::keyboard::modifiers::SerializedModifiers;
use crate::keyboard::xkb_state::Origin;
use crate::seat::Seat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Destination {
    Clients,
    Method,
}

impl Destination {
    pub(crate) fn for_key(seat: &Seat, source: KeySource) -> Self {
        if seat.ime.grab.is_some() && !matches!(source, KeySource::InputMethod(_)) {
            Self::Method
        } else {
            Self::Clients
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SentKeyboardState {
    pub(crate) origin: Origin,
    pub(crate) keymap: Keymap,
    pub(crate) modifiers: SerializedModifiers,
}

impl SentKeyboardState {
    fn update(
        &mut self,
        origin: Origin,
        keymap: &Keymap,
        modifiers: SerializedModifiers,
    ) -> KeyboardUpdate {
        self.origin = origin;
        let keymap = (self.keymap != *keymap).then(|| {
            self.keymap = keymap.clone();
            keymap.clone()
        });
        let changed = keymap.is_some() || self.modifiers != modifiers;
        self.modifiers = modifiers;
        KeyboardUpdate {
            keymap,
            modifiers: changed.then_some(modifiers),
        }
    }
}

struct KeyboardUpdate {
    keymap: Option<Keymap>,
    modifiers: Option<SerializedModifiers>,
}

pub(crate) fn present<A: Access>(
    state: &mut A,
    seat: SeatId,
    origin: Origin,
    destination: Destination,
) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(xkb_state) = entry.keyboard.xkb_state(origin) else {
        return;
    };
    let keymap = xkb_state.keymap.clone();
    let modifiers = xkb_state.serialized();
    let sent = match destination {
        Destination::Clients => entry.keyboard.sent.update(origin, &keymap, modifiers),
        Destination::Method => match entry.ime.grab.as_mut() {
            Some(grab) => grab.sent.update(origin, &keymap, modifiers),
            None => return,
        },
    };
    let Some(modifiers) = sent.modifiers else {
        return;
    };
    match destination {
        Destination::Clients => send_to_clients(state, seat, sent.keymap.as_ref(), modifiers),
        Destination::Method => send_to_input_method(state, seat, sent.keymap.as_ref(), modifiers),
    }
}

fn send_to_clients<A: Access>(
    state: &mut A,
    seat: SeatId,
    keymap: Option<&Keymap>,
    modifiers: SerializedModifiers,
) {
    let serial = state.display().next_serial();
    let seats = state.seats();
    let Some(client) = seats
        .get(seat)
        .and_then(|entry| entry.keyboard.focused_client())
        .filter(|client| seats.presents(seat, *client, Device::Keyboard))
    else {
        return;
    };
    let wire = seats.wire(seat, client);
    let Some(wired) = seats.get(wire) else {
        return;
    };
    for keyboard in wired.keyboard.keyboards.for_client(client) {
        if let Some(keymap) = keymap {
            send_keymap(keyboard, keymap);
        }
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

fn send_to_input_method<A: Access>(
    state: &mut A,
    seat: SeatId,
    keymap: Option<&Keymap>,
    modifiers: SerializedModifiers,
) {
    let serial = state.display().next_serial();
    let Some(grab) = state
        .seats()
        .get(seat)
        .and_then(|entry| entry.ime.grab.as_ref())
    else {
        return;
    };
    if let Some(keymap) = keymap {
        let (fd, size) = keymap.file();
        grab.resource.keymap(KeymapFormat::XkbV1, fd, size);
    }
    grab.resource.modifiers(
        serial,
        modifiers.depressed,
        modifiers.latched,
        modifiers.locked,
        modifiers.group,
    );
}

pub(crate) fn replace_origin_with_devices<A: Access>(
    state: &mut A,
    seat: SeatId,
    replaced: Origin,
) {
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    let clients = entry.keyboard.sent.origin == replaced;
    let method = entry
        .ime
        .grab
        .as_ref()
        .is_some_and(|grab| grab.sent.origin == replaced);
    if clients {
        present(state, seat, Origin::Devices, Destination::Clients);
    }
    if method {
        present(state, seat, Origin::Devices, Destination::Method);
    }
}

pub(crate) fn reselect<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let keyboard = &mut entry.keyboard;
    keyboard.physical.rekey(&keyboard.default);
    replace_origin_with_devices(state, seat, Origin::Devices);
    send_default_out_of_focus(state, seat);
}

pub(crate) fn set_repeat<A: Access>(state: &mut A, seat: SeatId, repeat: Repeat) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if entry.keyboard.repeat == repeat {
        return;
    }
    entry.keyboard.repeat = repeat;
    for keyboard in entry.keyboard.keyboards.all() {
        keyboard.repeat_info(repeat.rate, repeat.delay);
    }
    if let Some(grab) = &entry.ime.grab {
        grab.resource.repeat_info(repeat.rate, repeat.delay);
    }
}
