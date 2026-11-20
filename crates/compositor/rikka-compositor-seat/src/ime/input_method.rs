use rikka_compositor_protocols::wl_keyboard::KeymapFormat;
use rikka_compositor_protocols::zwp_input_method_keyboard_grab_v2::{
    self as grab_protocol, ZwpInputMethodKeyboardGrabV2,
};
use rikka_compositor_protocols::zwp_input_method_manager_v2::{
    self as manager, ZwpInputMethodManagerV2,
};
use rikka_compositor_protocols::zwp_input_method_v2::{self as protocol, ZwpInputMethodV2};

use crate::access::Access;
use crate::event::SeatEvent;
use crate::host::SeatHost;
use crate::ids::SeatId;
use crate::ime::composing;
use crate::ime::focus::{enter_focus, leave_focus};
use crate::ime::popup::open_popup;
use crate::ime::{Composition, InputMethod, MethodGrab};
use crate::keyboard::{KeyboardGrab, Origin, SentKeyboardState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Unavailable;

impl rikka_wayland_wire::Key for Unavailable {
    fn into_raw(self) -> u64 {
        0
    }

    fn from_raw(_raw: u64) -> Self {
        Self
    }
}

pub(crate) fn input_method_manager<A: Access>(
    state: &mut A,
    _manager: ZwpInputMethodManagerV2,
    request: manager::Request,
) {
    let manager::Request::GetInputMethod { seat, input_method } = request else {
        return;
    };
    let Some(seat) = seat.key::<SeatId>() else {
        return;
    };
    input_method.set_key(seat);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if entry
        .ime
        .method
        .as_ref()
        .is_some_and(|method| method.resource.alive())
    {
        input_method.set_key(Unavailable);
        input_method.unavailable();
        return;
    }
    entry.ime.method = Some(InputMethod {
        resource: input_method,
        pending: Composition::default(),
    });
    enter_focus(state, seat);
}

pub(crate) fn input_method_request<S: SeatHost>(
    state: &mut S,
    input_method: ZwpInputMethodV2,
    request: protocol::Request,
) {
    if input_method.key::<Unavailable>().is_some() {
        return;
    }
    let Some(seat) = input_method.key::<SeatId>() else {
        return;
    };
    match request {
        protocol::Request::Destroy => {}
        protocol::Request::CommitString { text } => {
            with_pending(state, seat, |pending| {
                pending.commit = Some(text.to_owned())
            });
        }
        protocol::Request::SetPreeditString {
            text,
            cursor_begin,
            cursor_end,
        } => with_pending(state, seat, |pending| {
            pending.preedit = Some((text.to_owned(), cursor_begin, cursor_end));
        }),
        protocol::Request::DeleteSurroundingText {
            before_length,
            after_length,
        } => with_pending(state, seat, |pending| {
            pending.delete = Some((before_length, after_length));
        }),
        protocol::Request::Commit { .. } => commit(state, seat),
        protocol::Request::GetInputPopupSurface { id, surface } => {
            open_popup(state, seat, &input_method, id, surface);
        }
        protocol::Request::GrabKeyboard { keyboard } => grab(state, seat, keyboard),
    }
}

pub(crate) fn input_method_destroyed<A: Access>(state: &mut A, input_method: ZwpInputMethodV2) {
    if input_method.key::<Unavailable>().is_some() {
        return;
    }
    let Some(seat) = input_method.key::<SeatId>() else {
        return;
    };
    let current = state
        .seats()
        .get(seat)
        .and_then(|entry| entry.ime.method.as_ref())
        .is_some_and(|method| method.resource == input_method);
    if !current {
        return;
    }
    leave_focus(state, seat);
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry.ime.method = None;
    }
}

pub(crate) fn grab_request<A: Access>(
    _state: &mut A,
    _grab: ZwpInputMethodKeyboardGrabV2,
    _request: grab_protocol::Request,
) {
}

pub(crate) fn grab_destroyed<A: Access>(state: &mut A, grab: ZwpInputMethodKeyboardGrabV2) {
    let Some(seat) = grab.key::<SeatId>() else {
        return;
    };
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    if entry
        .ime
        .grab
        .as_ref()
        .map(|current_grab| &current_grab.resource)
        != Some(&grab)
    {
        return;
    }
    entry.ime.grab = None;
    state.report(SeatEvent::KeyboardGrab {
        seat,
        grab: KeyboardGrab::InputMethod,
        active: false,
    });
}

fn with_pending<A: Access>(state: &mut A, seat: SeatId, visit: impl FnOnce(&mut Composition)) {
    if let Some(method) = state
        .seats()
        .seat_mut(seat)
        .and_then(|entry| entry.ime.method.as_mut())
    {
        visit(&mut method.pending);
    }
}

fn commit<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let Some(method) = entry.ime.method.as_mut() else {
        return;
    };
    let pending = std::mem::take(&mut method.pending);
    composing::deliver(state, seat, &pending);
}

fn grab<A: Access>(state: &mut A, seat: SeatId, keyboard: ZwpInputMethodKeyboardGrabV2) {
    keyboard.set_key(seat);
    let serial = state.display().next_serial();
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let physical = &entry.keyboard.physical;
    let sent = SentKeyboardState {
        origin: Origin::Devices,
        keymap: physical.keymap.clone(),
        modifiers: physical.serialized(),
    };
    let (fd, size) = sent.keymap.file();
    keyboard.keymap(KeymapFormat::XkbV1, fd, size);
    keyboard.repeat_info(entry.keyboard.repeat.rate, entry.keyboard.repeat.delay);
    keyboard.modifiers(
        serial,
        sent.modifiers.depressed,
        sent.modifiers.latched,
        sent.modifiers.locked,
        sent.modifiers.group,
    );
    entry.ime.grab = Some(MethodGrab {
        resource: keyboard,
        sent,
    });
    state.report(SeatEvent::KeyboardGrab {
        seat,
        grab: KeyboardGrab::InputMethod,
        active: true,
    });
}
