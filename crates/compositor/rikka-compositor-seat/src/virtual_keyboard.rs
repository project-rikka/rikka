use std::time::Duration;

use rikka_compositor_protocols::zwp_virtual_keyboard_manager_v1::{
    self as manager, ZwpVirtualKeyboardManagerV1,
};
use rikka_compositor_protocols::zwp_virtual_keyboard_v1::{self as protocol, ZwpVirtualKeyboardV1};
use rikka_compositor_types::Press;
use rikka_wayland_wire::ClientId;

use crate::access::Access;
use crate::host::SeatHost;
use crate::ids::{SeatId, VirtualKeyboardId};
use crate::input::{Key, KeySource};
use crate::keyboard::{self, Keymap, SerializedModifiers};

const XKB_V1: u32 = 1;

#[derive(Debug)]
pub(crate) struct VirtualKeyboard {
    pub(crate) seat: SeatId,
    pub(crate) client: ClientId,
    pub(crate) keymap: Option<Keymap>,
}

pub(crate) fn virtual_keyboard_manager<A: Access>(
    state: &mut A,
    _manager: ZwpVirtualKeyboardManagerV1,
    request: manager::Request,
) {
    let manager::Request::CreateVirtualKeyboard { seat, id } = request;
    let Some(seat) = seat.key::<SeatId>() else {
        return;
    };
    let board = state.seats().virtual_keyboards.insert(VirtualKeyboard {
        seat,
        client: id.client(),
        keymap: None,
    });
    id.set_key(board);
}

pub(crate) fn virtual_keyboard_request<S: SeatHost>(
    state: &mut S,
    keyboard: ZwpVirtualKeyboardV1,
    request: protocol::Request,
) {
    let Some(board) = keyboard.key::<VirtualKeyboardId>() else {
        return;
    };
    let Some((seat, typed)) = state
        .seats()
        .virtual_keyboards
        .get(board)
        .map(|keyboard| (keyboard.seat, keyboard.keymap.is_some()))
    else {
        return;
    };
    match request {
        protocol::Request::Destroy => {}
        protocol::Request::Keymap { format, fd, size } => {
            let keymap = if format == XKB_V1 {
                Keymap::from_fd(fd, size)
                    .inspect_err(|error| {
                        tracing::debug!("rejected virtual keyboard keymap: {error}");
                    })
                    .ok()
            } else {
                None
            };
            if let Some(board) = state.seats().virtual_keyboards.get_mut(board) {
                board.keymap = keymap;
            }
        }
        protocol::Request::Key {
            time,
            key,
            state: pressed,
        } => {
            if !typed {
                keyboard.post_error(protocol::Error::NoKeymap);
                return;
            }
            keyboard::key(
                state,
                seat,
                Key {
                    code: key,
                    press: if pressed == 0 {
                        Press::Released
                    } else {
                        Press::Pressed
                    },
                    time: Duration::from_millis(u64::from(time)),
                    source: KeySource::Virtual(board),
                },
            );
        }
        protocol::Request::Modifiers {
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
        } => {
            if !typed {
                keyboard.post_error(protocol::Error::NoKeymap);
                return;
            }
            keyboard::virtual_modifiers(
                state,
                seat,
                board,
                SerializedModifiers {
                    depressed: mods_depressed,
                    latched: mods_latched,
                    locked: mods_locked,
                    group,
                },
            );
        }
    }
}

pub(crate) fn virtual_keyboard_destroyed<A: Access>(state: &mut A, keyboard: ZwpVirtualKeyboardV1) {
    let Some(board) = keyboard.key::<VirtualKeyboardId>() else {
        return;
    };
    if !state.seats().virtual_keyboards.contains_key(board) {
        return;
    }
    let seats: Vec<SeatId> = state.seats().iter().map(|(seat, _)| seat).collect();
    for seat in seats {
        keyboard::forget_virtual(state, seat, board, Duration::ZERO);
    }
    state.seats().virtual_keyboards.remove(board);
}
