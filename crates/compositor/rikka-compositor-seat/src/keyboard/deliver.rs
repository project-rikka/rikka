use rikka_compositor_protocols::wl_keyboard::{KeymapFormat, WlKeyboard};

use crate::access::Access;
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
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let board = &mut entry.keyboard;
    let target = board
        .target
        .clone()
        .filter(|target| !board.withheld && target.client() == keyboard.client());
    let keymap = match target {
        Some(_) => &board.sent.keymap,
        None => &board.default,
    };
    send_keymap(&keyboard, keymap);
    keyboard.repeat_info(board.repeat.rate, board.repeat.delay);
    board.keyboards.add(keyboard.clone());
    let Some(target) = target else {
        return;
    };
    let pressed = pressed_array(&board.delivered());
    let modifiers = board.sent.modifiers;
    let serial = state.display().next_serial();
    keyboard.enter(serial, &target, &pressed);
    keyboard.modifiers(
        serial,
        modifiers.depressed,
        modifiers.latched,
        modifiers.locked,
        modifiers.group,
    );
}

pub(crate) fn send_default_out_of_focus<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    let board = &entry.keyboard;
    let focused = board.focused_client();
    for keyboard in board.keyboards.all() {
        if Some(keyboard.client()) != focused {
            send_keymap(keyboard, &board.default);
        }
    }
}
