use rikka_compositor_protocols::wl_pointer::{
    Axis as WireAxis, AxisRelativeDirection, AxisSource as WireSource, ButtonState, WlPointer,
};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::{AxisSource, Gesture, Press};
use rikka_wayland_wire::ClientId;

use crate::access::Access;
use crate::focus::{Focus, Focused};
use crate::ids::SeatId;
use crate::input::millis;
use crate::pointer::constraint::{self, deactivate_constraints_except};
use crate::pointer::cursor::{CursorImage, CursorOwner, show};
use crate::pointer::events::{Axis, AxisMotion, Button, Motion, RelativeMotion};
use crate::pointer::{ActiveGesture, GestureKind, Pointer};
use crate::surface;

const WARP_SINCE: u32 = 11;

pub(crate) fn deliver_motion<A: Access>(state: &mut A, seat: SeatId, motion: &Motion) {
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    let pointer = &entry.pointer;
    let target = if pointer.buttons.is_empty() {
        motion.under
    } else {
        pointer.focus()
    };
    deactivate_constraints_except(state, seat, target.map(|focus| focus.surface));
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    if entry.pointer.focused_surface() != target.map(|focus| focus.surface) {
        change_focus(state, seat, target, motion);
    }
    if let Some(focus) = target {
        move_within(state, seat, focus, motion);
    }
    constraint::refresh(state, seat);
}

fn move_within<A: Access>(state: &mut A, seat: SeatId, focus: Focus, motion: &Motion) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    let locked = locked(pointer);
    let Some(focused) = pointer.focused.as_mut() else {
        return;
    };
    focused.focus = focus;
    if locked {
        return;
    }
    let client = focused.client();
    let local = focus.local(motion.location);
    for resource in pointer.pointers.for_client(client) {
        resource.motion(millis(motion.time), local.x, local.y);
    }
    pointer.mark(client);
}

pub(crate) fn change_focus<A: Access>(
    state: &mut A,
    seat: SeatId,
    target: Option<Focus>,
    motion: &Motion,
) {
    let serial = state.display().next_serial();
    let entered = target.and_then(|focus| {
        surface::resource(state, focus.surface).map(|surface| Focused { focus, surface })
    });
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    if let Some(old) = pointer.focused.take()
        && old.surface.alive()
    {
        let client = old.client();
        for resource in pointer.pointers.for_client(client) {
            resource.leave(serial, &old.surface);
        }
        pointer.mark(client);
    }
    if let Some(focused) = &entered {
        let local = focused.focus.local(motion.location);
        let client = focused.client();
        for resource in pointer.pointers.for_client(client) {
            resource.enter(serial, &focused.surface, local.x, local.y);
        }
        pointer.mark(client);
        pointer.enter_serial = serial;
    }
    pointer.focused = entered;
    show(
        state,
        seat,
        CursorOwner::Pointer,
        CursorImage::default(),
        None,
    );
}

pub(crate) fn refocus<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    let pointer = &entry.pointer;
    if !pointer.buttons.is_empty() {
        return;
    }
    let motion = Motion {
        location: pointer.location,
        under: pointer.under,
        time: std::time::Duration::ZERO,
    };
    if pointer.focused_surface() == motion.under.map(|focus| focus.surface) {
        return;
    }
    change_focus(state, seat, motion.under, &motion);
    constraint::refresh(state, seat);
}

pub(crate) fn deliver_warp<A: Access>(state: &mut A, seat: SeatId, motion: &Motion) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    pointer.location = motion.location;
    pointer.under = motion.under;
    let current = pointer.focus();
    let target = match current {
        Some(focus) if !pointer.buttons.is_empty() => Some(
            motion
                .under
                .filter(|under| under.surface == focus.surface)
                .unwrap_or(focus),
        ),
        _ => motion.under,
    };
    if current.map(|focus| focus.surface) != target.map(|focus| focus.surface) {
        deactivate_constraints_except(state, seat, target.map(|focus| focus.surface));
        change_focus(state, seat, target, motion);
        constraint::refresh(state, seat);
        deliver_frame(state, seat);
        return;
    }
    let Some(focus) = target else {
        return;
    };
    let locked = locked(pointer);
    let Some(focused) = pointer.focused.as_mut() else {
        return;
    };
    focused.focus = focus;
    let client = focused.client();
    if let Some(index) = pointer
        .unframed
        .iter()
        .position(|unframed| *unframed == client)
    {
        pointer.unframed.remove(index);
        for resource in pointer.pointers.for_client(client) {
            resource.frame();
        }
    }
    let local = focus.local(motion.location);
    for resource in pointer.pointers.for_client(client) {
        if resource.version() >= WARP_SINCE {
            resource.warp(local.x, local.y);
        } else if locked {
            continue;
        } else {
            resource.motion(millis(motion.time), local.x, local.y);
        }
        resource.frame();
    }
    constraint::refresh(state, seat);
}

pub(crate) fn deliver_button<A: Access>(state: &mut A, seat: SeatId, button: &Button, serial: u32) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    let Some(client) = focused_client(pointer) else {
        return;
    };
    let pressed = match button.press {
        Press::Pressed => ButtonState::Pressed,
        Press::Released => ButtonState::Released,
    };
    for resource in pointer.pointers.for_client(client) {
        resource.button(serial, millis(button.time), button.code, pressed);
    }
    pointer.mark(client);
    if button.press == Press::Released && pointer.buttons.is_empty() && !pointer.grab.grabbed() {
        refocus(state, seat);
    }
}

pub(crate) fn deliver_axis<A: Access>(state: &mut A, seat: SeatId, axis: &Axis) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    let Some(client) = focused_client(pointer) else {
        return;
    };
    let time = millis(axis.time);
    for resource in pointer.pointers.for_client(client) {
        if let Some(source) = axis.source {
            resource.axis_source(wire_source(source));
        }
        for (wire, motion) in [
            (WireAxis::HorizontalScroll, axis.horizontal),
            (WireAxis::VerticalScroll, axis.vertical),
        ] {
            send_axis(resource, wire, &motion, time);
        }
    }
    pointer.mark(client);
}

fn send_axis(pointer: &WlPointer, axis: WireAxis, motion: &AxisMotion, time: u32) {
    if motion.is_empty() {
        return;
    }
    if motion.moved() {
        pointer.axis_relative_direction(
            axis,
            if motion.inverted {
                AxisRelativeDirection::Inverted
            } else {
                AxisRelativeDirection::Identical
            },
        );
        if let Some(v120) = motion.v120 {
            if pointer.version() >= 8 {
                pointer.axis_value120(axis, v120);
            } else if v120 / 120 != 0 {
                pointer.axis_discrete(axis, v120 / 120);
            }
        }
        pointer.axis(time, axis, motion.value);
    }
    if motion.stopped {
        pointer.axis_stop(time, axis);
    }
}

fn wire_source(source: AxisSource) -> WireSource {
    match source {
        AxisSource::Wheel => WireSource::Wheel,
        AxisSource::Finger => WireSource::Finger,
        AxisSource::Continuous => WireSource::Continuous,
        AxisSource::WheelTilt => WireSource::WheelTilt,
    }
}

pub(crate) fn deliver_frame<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    for client in pointer.unframed.drain(..) {
        for resource in pointer.pointers.for_client(client) {
            resource.frame();
        }
    }
}

pub(crate) fn deliver_relative<A: Access>(state: &mut A, seat: SeatId, motion: &RelativeMotion) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    let Some(client) = focused_client(pointer) else {
        return;
    };
    let micros = motion.time.as_micros() as u64;
    let (high, low) = ((micros >> 32) as u32, micros as u32);
    for relative in pointer.relative.for_client(client) {
        relative.relative_motion(
            high,
            low,
            motion.delta.x,
            motion.delta.y,
            motion.unaccelerated.x,
            motion.unaccelerated.y,
        );
    }
    pointer.mark(client);
}

pub(crate) fn deliver_gesture<A: Access>(
    state: &mut A,
    seat: SeatId,
    gesture: &Gesture,
    serial: u32,
) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    let begin = match *gesture {
        Gesture::SwipeBegin { fingers, time } => Some((GestureKind::Swipe, fingers, time)),
        Gesture::PinchBegin { fingers, time } => Some((GestureKind::Pinch, fingers, time)),
        Gesture::HoldBegin { fingers, time } => Some((GestureKind::Hold, fingers, time)),
        _ => None,
    };
    if let Some((kind, fingers, time)) = begin {
        let Some(focused) = &pointer.focused else {
            return;
        };
        let client = focused.client();
        let time = millis(time);
        let wl = &focused.surface;
        match kind {
            GestureKind::Swipe => {
                for swipe in pointer.swipes.for_client(client) {
                    swipe.begin(serial, time, wl, fingers);
                }
            }
            GestureKind::Pinch => {
                for pinch in pointer.pinches.for_client(client) {
                    pinch.begin(serial, time, wl, fingers);
                }
            }
            GestureKind::Hold => {
                for hold in pointer.holds.for_client(client) {
                    hold.begin(serial, time, wl, fingers);
                }
            }
        }
        pointer.gesture = Some(ActiveGesture {
            kind,
            surface: focused.focus.surface,
            client,
        });
        pointer.mark(client);
        return;
    }
    let Some(gesturing) = pointer.gesture else {
        return;
    };
    let client = gesturing.client;
    match (*gesture, gesturing.kind) {
        (Gesture::SwipeUpdate { delta, time }, GestureKind::Swipe) => {
            for swipe in pointer.swipes.for_client(client) {
                swipe.update(millis(time), delta.x, delta.y);
            }
        }
        (Gesture::SwipeEnd { cancelled, time }, GestureKind::Swipe) => {
            for swipe in pointer.swipes.for_client(client) {
                swipe.end(serial, millis(time), i32::from(cancelled));
            }
            pointer.gesture = None;
        }
        (
            Gesture::PinchUpdate {
                delta,
                scale,
                rotation,
                time,
            },
            GestureKind::Pinch,
        ) => {
            for pinch in pointer.pinches.for_client(client) {
                pinch.update(millis(time), delta.x, delta.y, scale, rotation);
            }
        }
        (Gesture::PinchEnd { cancelled, time }, GestureKind::Pinch) => {
            for pinch in pointer.pinches.for_client(client) {
                pinch.end(serial, millis(time), i32::from(cancelled));
            }
            pointer.gesture = None;
        }
        (Gesture::HoldEnd { cancelled, time }, GestureKind::Hold) => {
            for hold in pointer.holds.for_client(client) {
                hold.end(serial, millis(time), i32::from(cancelled));
            }
            pointer.gesture = None;
        }
        _ => return,
    }
    pointer.mark(client);
}

pub(crate) fn pointer_surface_gone<A: Access>(state: &mut A, seat: SeatId, surface: SurfaceId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    if pointer.under.is_some_and(|under| under.surface == surface) {
        pointer.under = None;
    }
    if pointer
        .gesture
        .is_some_and(|gesturing| gesturing.surface == surface)
    {
        pointer.gesture = None;
    }
    if pointer
        .press
        .is_some_and(|press| press.focus.is_some_and(|focus| focus.surface == surface))
    {
        pointer.press = None;
    }
    if pointer.focused_surface() == Some(surface) {
        pointer.focused = None;
        show(
            state,
            seat,
            CursorOwner::Pointer,
            CursorImage::default(),
            None,
        );
    }
}

pub(crate) fn introduce<A: Access>(state: &mut A, seat: SeatId, resource: WlPointer) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    pointer.pointers.add(resource.clone());
    let Some(focused) = &pointer.focused else {
        return;
    };
    if focused.client() != resource.client() {
        return;
    }
    let local = focused.focus.local(pointer.location);
    resource.enter(pointer.enter_serial, &focused.surface, local.x, local.y);
    resource.frame();
}

fn focused_client(pointer: &Pointer) -> Option<ClientId> {
    pointer.focused.as_ref().map(Focused::client)
}

fn locked(pointer: &Pointer) -> bool {
    pointer
        .constraint()
        .is_some_and(|(_, kind)| kind == constraint::ConstraintKind::Lock)
}
