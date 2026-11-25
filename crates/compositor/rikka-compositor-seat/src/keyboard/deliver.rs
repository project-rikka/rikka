use rikka_compositor_protocols::wl_keyboard::{KeymapFormat, WlKeyboard};

use crate::access::Access;
use crate::carry::Device;
use crate::ids::SeatId;
use crate::keyboard::keymap::{Keymap, KeymapStamp};

pub(crate) fn send_keymap(keyboard: &WlKeyboard, keymap: &Keymap) {
    if keyboard.key::<KeymapStamp>() == Some(keymap.stamp()) {
        return;
    }
    let (fd, size) = keymap.file();
    keyboard.keymap(KeymapFormat::XkbV1, fd, size);
    keyboard.set_key(keymap.stamp());
}

pub(crate) fn pressed_array(codes: &[u32]) -> Vec<u8> {
    codes.iter().flat_map(|code| code.to_ne_bytes()).collect()
}

pub(crate) fn introduce<A: Access>(state: &mut A, seat: SeatId, keyboard: WlKeyboard) {
    let serial = state.display().next_serial();
    let client = keyboard.client();
    let seats = state.seats();
    let presenter = seats.presenter(seat, client, Device::Keyboard);
    let focused = seats.focuses(presenter, client, Device::Keyboard);
    let (Some(entry), Some(shown)) = (seats.get(seat), seats.get(presenter)) else {
        return;
    };
    let board = &entry.keyboard;
    let shown = &shown.keyboard;
    let keymap = match focused {
        true => &shown.sent.keymap,
        false => &board.default,
    };
    send_keymap(&keyboard, keymap);
    keyboard.repeat_info(board.repeat.rate, board.repeat.delay);
    if let Some(target) = shown.target.as_ref().filter(|_| focused) {
        let pressed = pressed_array(&shown.delivered());
        let modifiers = shown.sent.modifiers;
        keyboard.enter(serial, target, &pressed);
        keyboard.modifiers(
            serial,
            modifiers.depressed,
            modifiers.latched,
            modifiers.locked,
            modifiers.group,
        );
        seats.carry_serial(presenter, seat, serial);
    }
    if let Some(entry) = seats.seat_mut(seat) {
        entry.keyboard.keyboards.add(keyboard);
    }
}

pub(crate) fn send_default_out_of_focus<A: Access>(state: &mut A, seat: SeatId) {
    let seats = state.seats();
    let Some(entry) = seats.get(seat) else {
        return;
    };
    let board = &entry.keyboard;
    for keyboard in board.keyboards.all() {
        let client = keyboard.client();
        let presenter = seats.presenter(seat, client, Device::Keyboard);
        if !seats.focuses(presenter, client, Device::Keyboard) {
            send_keymap(keyboard, &board.default);
        }
    }
}
