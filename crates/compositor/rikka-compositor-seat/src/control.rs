use std::fmt;
use std::time::Duration;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::{Gesture, PadInput, Press, ToolInfo};

use crate::access::SeatAccess;
use crate::focus::KeyboardFocus;
use crate::host::SeatHost;
use crate::ids::{PadId, SeatId, TabletId, ToolId};
use crate::ime::Composition;
use crate::input::Key;
use crate::keyboard::{self, Keymap, Repeat};
use crate::pointer::{
    self, Axis, Button, GrabFocus, GrabStart, Motion, PointerGrab, RelativeMotion,
};
use crate::seat::Capabilities;
use crate::tablet::{self, PadInfo, TabletInfo, ToolMotion};
use crate::touch::{self, TouchDown, TouchGrab, TouchMotion, TouchShape, TouchUp};
use crate::{idle, ime};

pub fn seat<A: SeatAccess>(state: &mut A, seat: SeatId) -> SeatMut<'_, A> {
    SeatMut {
        state: Held::Borrowed(state),
        seat,
    }
}

pub fn set_idle_inhibited<A: SeatAccess>(state: &mut A, inhibited: bool) {
    idle::set_inhibited(state, inhibited);
}

pub struct SeatMut<'a, A> {
    state: Held<'a, A>,
    seat: SeatId,
}

enum Held<'a, A> {
    Borrowed(&'a mut A),
    Owned(A),
}

fn held<'h, A>(held: &'h mut Held<'_, A>) -> &'h mut A {
    match held {
        Held::Borrowed(state) => state,
        Held::Owned(state) => state,
    }
}

impl<A> SeatMut<'_, A> {
    pub(crate) fn owning(state: A, seat: SeatId) -> Self {
        Self {
            state: Held::Owned(state),
            seat,
        }
    }
}

impl<A: SeatAccess> SeatMut<'_, A> {
    pub fn id(&self) -> SeatId {
        self.seat
    }

    pub fn set_capabilities(&mut self, capabilities: Capabilities) {
        let Some(entry) = held(&mut self.state).seats().seat_mut(self.seat) else {
            return;
        };
        if entry.capabilities == capabilities {
            return;
        }
        entry.capabilities = capabilities;
        for handle in entry.handles.iter().filter(|handle| handle.alive()) {
            handle.capabilities(capabilities.wire());
        }
    }

    pub fn set_keyboard_focus(&mut self, focus: Option<KeyboardFocus>) -> bool {
        keyboard::set_focus(held(&mut self.state), self.seat, focus, false)
    }

    pub fn withhold_keyboard(&mut self) {
        keyboard::withhold(held(&mut self.state), self.seat);
    }

    pub fn restore_keyboard(&mut self) {
        keyboard::restore(held(&mut self.state), self.seat);
    }

    pub fn set_keymap(&mut self, keymap: Keymap) {
        let Some(entry) = held(&mut self.state).seats().seat_mut(self.seat) else {
            return;
        };
        entry.keyboard.default = keymap;
        keyboard::reselect(held(&mut self.state), self.seat);
    }

    pub fn set_repeat(&mut self, repeat: Repeat) {
        keyboard::set_repeat(held(&mut self.state), self.seat, repeat);
    }

    pub fn warp(&mut self, motion: Motion) {
        pointer::deliver_warp(held(&mut self.state), self.seat, &motion);
    }

    pub fn constrain(&mut self, proposed: Point<f64, Logical>) -> Point<f64, Logical> {
        pointer::constrain(held(&mut self.state), self.seat, proposed)
    }

    pub fn set_pointer_grab(&mut self, grab: impl PointerGrab<A::Host>, focus: GrabFocus) {
        pointer::set_grab(held(&mut self.state), self.seat, grab, focus);
    }

    pub fn end_pointer_grab(&mut self) {
        pointer::end_grab(held(&mut self.state), self.seat);
    }

    pub fn set_touch_grab(&mut self, grab: impl TouchGrab<A::Host>, focus: GrabFocus) {
        touch::set_grab(held(&mut self.state), self.seat, grab, focus);
    }

    pub fn end_touch_grab(&mut self) {
        touch::end_grab(held(&mut self.state), self.seat);
    }

    pub fn add_tablet(&mut self, info: TabletInfo) -> Option<TabletId> {
        tablet::add_tablet(held(&mut self.state), self.seat, info)
    }

    pub fn remove_tablet(&mut self, tablet: TabletId) {
        tablet::remove_tablet(held(&mut self.state), self.seat, tablet);
    }

    pub fn add_tool(&mut self, info: ToolInfo) -> Option<ToolId> {
        tablet::add_tool(held(&mut self.state), self.seat, info)
    }

    pub fn remove_tool(&mut self, tool: ToolId) {
        tablet::remove_tool(held(&mut self.state), self.seat, tool);
    }

    pub fn add_pad(&mut self, tablet: TabletId, info: PadInfo) -> Option<PadId> {
        tablet::add_pad(held(&mut self.state), self.seat, tablet, info)
    }

    pub fn remove_pad(&mut self, pad: PadId) {
        tablet::remove_pad(held(&mut self.state), self.seat, pad);
    }

    pub fn tool_motion(&mut self, tool: ToolId, tablet: TabletId, motion: ToolMotion) {
        tablet::tool_motion(held(&mut self.state), self.seat, tool, tablet, &motion);
    }

    pub fn tool_out(&mut self, tool: ToolId, time: Duration) {
        tablet::tool_out(held(&mut self.state), self.seat, tool, time);
    }

    pub fn tool_tip(&mut self, tool: ToolId, press: Press, time: Duration) {
        tablet::tool_tip(held(&mut self.state), self.seat, tool, press, time);
    }

    pub fn tool_button(&mut self, tool: ToolId, button: u32, press: Press, time: Duration) {
        tablet::tool_button(held(&mut self.state), self.seat, tool, button, press, time);
    }

    pub fn pad_focus(&mut self, pad: PadId, surface: Option<SurfaceId>) {
        tablet::pad_focus(held(&mut self.state), self.seat, pad, surface);
    }

    pub fn pad_input(&mut self, pad: PadId, input: PadInput, time: Duration) {
        tablet::pad_input(held(&mut self.state), self.seat, pad, input, time);
    }

    pub fn place_input_popup(&mut self, popup: SurfaceId, at: Point<i32, Logical>) {
        ime::place_popup(held(&mut self.state), self.seat, popup, at);
    }

    pub fn serve_text_input(&mut self, served: bool) {
        ime::serve_text_input(held(&mut self.state), self.seat, served);
    }

    pub fn compose(&mut self, composition: &Composition) {
        ime::compose(held(&mut self.state), self.seat, composition);
    }

    pub fn activity(&mut self) {
        idle::activity(held(&mut self.state), self.seat);
    }
}

impl<S: SeatHost> SeatMut<'_, S> {
    pub fn key(&mut self, key: Key) {
        keyboard::key(held(&mut self.state), self.seat, key);
    }

    pub fn motion(&mut self, motion: Motion) {
        let Some(entry) = held(&mut self.state).seats().seat_mut(self.seat) else {
            return;
        };
        entry.pointer.location = motion.location;
        entry.pointer.under = motion.under;
        let grabbed = pointer::dispatch(held(&mut self.state), self.seat, 0, |grab, handle| {
            grab.motion(handle, &motion);
        });
        if !grabbed {
            pointer::deliver_motion(held(&mut self.state), self.seat, &motion);
        }
    }

    pub fn relative_motion(&mut self, motion: RelativeMotion) {
        let grabbed = pointer::dispatch(held(&mut self.state), self.seat, 0, |grab, handle| {
            grab.relative_motion(handle, &motion);
        });
        if !grabbed {
            pointer::deliver_relative(held(&mut self.state), self.seat, &motion);
        }
    }

    pub fn button(&mut self, button: Button) -> u32 {
        let serial = held(&mut self.state).display().next_serial();
        let Some(entry) = held(&mut self.state).seats().seat_mut(self.seat) else {
            return serial;
        };
        let pointer = &mut entry.pointer;
        match button.press {
            Press::Pressed => {
                if pointer.buttons.is_empty() {
                    pointer.press = Some(GrabStart {
                        serial,
                        button: button.code,
                        location: pointer.location,
                        focus: pointer.focus(),
                    });
                }
                if !pointer.buttons.contains(&button.code) {
                    pointer.buttons.push(button.code);
                }
            }
            Press::Released => pointer.buttons.retain(|code| *code != button.code),
        }
        let grabbed =
            pointer::dispatch(held(&mut self.state), self.seat, serial, |grab, handle| {
                grab.button(handle, &button);
            });
        if !grabbed {
            pointer::deliver_button(held(&mut self.state), self.seat, &button, serial);
        }
        serial
    }

    pub fn axis(&mut self, axis: Axis) {
        let grabbed = pointer::dispatch(held(&mut self.state), self.seat, 0, |grab, handle| {
            grab.axis(handle, &axis);
        });
        if !grabbed {
            pointer::deliver_axis(held(&mut self.state), self.seat, &axis);
        }
    }

    pub fn gesture(&mut self, gesture: Gesture) {
        let serial = held(&mut self.state).display().next_serial();
        let grabbed =
            pointer::dispatch(held(&mut self.state), self.seat, serial, |grab, handle| {
                grab.gesture(handle, &gesture);
            });
        if !grabbed {
            pointer::deliver_gesture(held(&mut self.state), self.seat, &gesture, serial);
        }
    }

    pub fn frame(&mut self) {
        let grabbed = pointer::dispatch(held(&mut self.state), self.seat, 0, |grab, handle| {
            grab.frame(handle);
        });
        if !grabbed {
            pointer::deliver_frame(held(&mut self.state), self.seat);
        }
    }

    pub fn touch_down(&mut self, down: TouchDown) {
        if !touch::dispatch(held(&mut self.state), self.seat, |grab, handle| {
            grab.down(handle, &down)
        }) {
            touch::touch_down(held(&mut self.state), self.seat, &down);
        }
    }

    pub fn touch_motion(&mut self, motion: TouchMotion) {
        if !touch::dispatch(held(&mut self.state), self.seat, |grab, handle| {
            grab.motion(handle, &motion);
        }) {
            touch::touch_motion(held(&mut self.state), self.seat, &motion);
        }
    }

    pub fn touch_up(&mut self, up: TouchUp) {
        if !touch::dispatch(held(&mut self.state), self.seat, |grab, handle| {
            grab.up(handle, &up)
        }) {
            touch::touch_up(held(&mut self.state), self.seat, &up);
        }
    }

    pub fn touch_shape(&mut self, shape: TouchShape) {
        if !touch::dispatch(held(&mut self.state), self.seat, |grab, handle| {
            grab.shape(handle, &shape);
        }) {
            touch::touch_shape(held(&mut self.state), self.seat, &shape);
        }
    }

    pub fn touch_frame(&mut self) {
        if !touch::dispatch(held(&mut self.state), self.seat, |grab, handle| {
            grab.frame(handle)
        }) {
            touch::touch_frame(held(&mut self.state), self.seat);
        }
    }

    pub fn touch_cancel(&mut self) {
        if !touch::dispatch(held(&mut self.state), self.seat, |grab, handle| {
            grab.cancel(handle)
        }) {
            touch::touch_cancel(held(&mut self.state), self.seat);
        }
    }
}

impl<A> fmt::Debug for SeatMut<'_, A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SeatMut")
            .field("seat", &self.seat)
            .finish_non_exhaustive()
    }
}
