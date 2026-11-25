use rikka_compositor_protocols::wl_pointer::{
    Axis as WireAxis, AxisRelativeDirection, AxisSource as WireSource, ButtonState, WlPointer,
};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::{AxisSource, Gesture, Press};
use rikka_wayland_wire::ClientId;

use crate::access::Access;
use crate::carry::{self, Claim, Device};
use crate::focus::{Focus, Focused};
use crate::ids::SeatId;
use crate::input::millis;
use crate::pointer::constraint::{self, deactivate_constraints_except};
use crate::pointer::cursor::{CursorImage, CursorOwner, show};
use crate::pointer::events::{Axis, AxisMotion, Button, Motion, RelativeMotion};
use crate::pointer::{ActiveGesture, GestureKind, Pointer};
use crate::seats::Seats;
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
        change_focus(state, seat, target, Claim::Active);
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
    let Some(focused) = entry.pointer.focused.as_mut() else {
        return;
    };
    focused.focus = focus;
    let client = focused.client();
    if !carry::claim(state, seat, client, Device::Pointer, Claim::Active) {
        return;
    }
    let seats = state.seats();
    if seats.get(seat).is_none_or(|entry| locked(&entry.pointer)) {
        return;
    }
    let wire = seats.wire(seat, client);
    let local = focus.local(motion.location);
    for resource in pointers(seats, wire, client) {
        resource.motion(millis(motion.time), local.x, local.y);
    }
    mark(seats, seat, wire, client);
}

pub(crate) fn change_focus<A: Access>(
    state: &mut A,
    seat: SeatId,
    target: Option<Focus>,
    claim: Claim,
) {
    let entered = target.and_then(|focus| {
        surface::resource(state, focus.surface).map(|surface| Focused { focus, surface })
    });
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    let left = focused_client(&entry.pointer);
    if let Some(client) = left
        && state.seats().presents(seat, client, Device::Pointer)
    {
        conceal(state, seat, client, seat);
    }
    let joined = entered.as_ref().map(Focused::client);
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    entry.pointer.focused = entered;
    if let Some(client) = left
        && joined != Some(client)
    {
        carry::vacate(state, seat, client, Device::Pointer);
    }
    if let Some(client) = joined {
        carry::enter(state, seat, client, Device::Pointer, claim);
    }
    show(
        state,
        seat,
        CursorOwner::Pointer,
        CursorImage::default(),
        None,
    );
}

pub(crate) fn reveal<A: Access>(state: &mut A, seat: SeatId, client: ClientId, actor: SeatId) {
    let serial = state.display().next_serial();
    let seats = state.seats();
    let wire = seats.wire(seat, client);
    let Some(entry) = seats.get(seat) else {
        return;
    };
    let pointer = &entry.pointer;
    let Some(focused) = pointer
        .focused
        .as_ref()
        .filter(|focused| focused.client() == client && focused.surface.alive())
    else {
        return;
    };
    let local = focused.focus.local(pointer.location);
    let surface = focused.surface.clone();
    for resource in pointers(seats, wire, client) {
        resource.enter(serial, &surface, local.x, local.y);
    }
    seats.carry_serial(seat, wire, serial);
    if let Some(entry) = seats.seat_mut(seat) {
        entry.pointer.enter_serial = serial;
    }
    mark(seats, actor, wire, client);
}

pub(crate) fn conceal<A: Access>(state: &mut A, seat: SeatId, client: ClientId, actor: SeatId) {
    let serial = state.display().next_serial();
    let seats = state.seats();
    let wire = seats.wire(seat, client);
    let Some(surface) = seats
        .get(seat)
        .and_then(|entry| entry.pointer.focused.as_ref())
        .filter(|focused| focused.client() == client && focused.surface.alive())
        .map(|focused| focused.surface.clone())
    else {
        return;
    };
    for resource in pointers(seats, wire, client) {
        resource.leave(serial, &surface);
    }
    seats.carry_serial(seat, wire, serial);
    mark(seats, actor, wire, client);
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
    let under = pointer.under;
    if pointer.focused_surface() == under.map(|focus| focus.surface) {
        return;
    }
    change_focus(state, seat, under, Claim::Passive);
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
        change_focus(state, seat, target, Claim::Passive);
        constraint::refresh(state, seat);
        deliver_frame(state, seat);
        return;
    }
    let Some(focus) = target else {
        return;
    };
    let Some(focused) = pointer.focused.as_mut() else {
        return;
    };
    focused.focus = focus;
    let client = focused.client();
    let locked = locked(pointer);
    let seats = state.seats();
    if seats.presents(seat, client, Device::Pointer) {
        let wire = seats.wire(seat, client);
        if let Some(entry) = seats.seat_mut(seat)
            && let Some(index) = entry
                .pointer
                .unframed
                .iter()
                .position(|unframed| *unframed == (wire, client))
        {
            entry.pointer.unframed.remove(index);
            for resource in pointers(seats, wire, client) {
                resource.frame();
            }
        }
        let local = focus.local(motion.location);
        for resource in pointers(seats, wire, client) {
            if resource.version() >= WARP_SINCE {
                resource.warp(local.x, local.y);
            } else if locked {
                continue;
            } else {
                resource.motion(millis(motion.time), local.x, local.y);
            }
            resource.frame();
        }
    }
    constraint::refresh(state, seat);
}

pub(crate) fn deliver_button<A: Access>(state: &mut A, seat: SeatId, button: &Button, serial: u32) {
    let Some(client) = state
        .seats()
        .get(seat)
        .and_then(|entry| focused_client(&entry.pointer))
    else {
        return;
    };
    let presented = match button.press {
        Press::Pressed => carry::claim(state, seat, client, Device::Pointer, Claim::Active),
        Press::Released => state.seats().presents(seat, client, Device::Pointer),
    };
    if presented {
        let pressed = match button.press {
            Press::Pressed => ButtonState::Pressed,
            Press::Released => ButtonState::Released,
        };
        let seats = state.seats();
        let wire = seats.wire(seat, client);
        for resource in pointers(seats, wire, client) {
            resource.button(serial, millis(button.time), button.code, pressed);
        }
        seats.carry_serial(seat, wire, serial);
        mark(seats, seat, wire, client);
    }
    let Some(entry) = state.seats().get(seat) else {
        return;
    };
    let pointer = &entry.pointer;
    if button.press == Press::Released && pointer.buttons.is_empty() && !pointer.grab.grabbed() {
        refocus(state, seat);
    }
}

pub(crate) fn deliver_axis<A: Access>(state: &mut A, seat: SeatId, axis: &Axis) {
    let Some(client) = state
        .seats()
        .get(seat)
        .and_then(|entry| focused_client(&entry.pointer))
    else {
        return;
    };
    if !carry::claim(state, seat, client, Device::Pointer, Claim::Active) {
        return;
    }
    let seats = state.seats();
    let wire = seats.wire(seat, client);
    let time = millis(axis.time);
    for resource in pointers(seats, wire, client) {
        if let Some(source) = axis.source {
            resource.axis_source(wire_source(source));
        }
        for (wire_axis, motion) in [
            (WireAxis::HorizontalScroll, axis.horizontal),
            (WireAxis::VerticalScroll, axis.vertical),
        ] {
            send_axis(resource, wire_axis, &motion, time);
        }
    }
    mark(seats, seat, wire, client);
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
    let seats = state.seats();
    let Some(entry) = seats.seat_mut(seat) else {
        return;
    };
    let unframed = std::mem::take(&mut entry.pointer.unframed);
    for (wire, client) in unframed {
        for resource in pointers(seats, wire, client) {
            resource.frame();
        }
    }
}

pub(crate) fn deliver_relative<A: Access>(state: &mut A, seat: SeatId, motion: &RelativeMotion) {
    let seats = state.seats();
    let Some(client) = seats
        .get(seat)
        .and_then(|entry| focused_client(&entry.pointer))
    else {
        return;
    };
    if !seats.presents(seat, client, Device::Pointer) {
        return;
    }
    let wire = seats.wire(seat, client);
    let micros = motion.time.as_micros() as u64;
    let (high, low) = ((micros >> 32) as u32, micros as u32);
    if let Some(entry) = seats.get(wire) {
        for relative in entry.pointer.relative.for_client(client) {
            relative.relative_motion(
                high,
                low,
                motion.delta.x,
                motion.delta.y,
                motion.unaccelerated.x,
                motion.unaccelerated.y,
            );
        }
    }
    mark(seats, seat, wire, client);
}

pub(crate) fn deliver_gesture<A: Access>(
    state: &mut A,
    seat: SeatId,
    gesture: &Gesture,
    serial: u32,
) {
    let begin = match *gesture {
        Gesture::SwipeBegin { fingers, time } => Some((GestureKind::Swipe, fingers, time)),
        Gesture::PinchBegin { fingers, time } => Some((GestureKind::Pinch, fingers, time)),
        Gesture::HoldBegin { fingers, time } => Some((GestureKind::Hold, fingers, time)),
        _ => None,
    };
    if let Some((kind, fingers, time)) = begin {
        begin_gesture(state, seat, kind, fingers, millis(time), serial);
        return;
    }
    let seats = state.seats();
    let Some(gesturing) = seats.get(seat).and_then(|entry| entry.pointer.gesture) else {
        return;
    };
    let client = gesturing.client;
    let wire = seats.wire(seat, client);
    let Some(entry) = seats.get(wire) else {
        return;
    };
    let pointer = &entry.pointer;
    let ended = match (*gesture, gesturing.kind) {
        (Gesture::SwipeUpdate { delta, time }, GestureKind::Swipe) => {
            for swipe in pointer.swipes.for_client(client) {
                swipe.update(millis(time), delta.x, delta.y);
            }
            false
        }
        (Gesture::SwipeEnd { cancelled, time }, GestureKind::Swipe) => {
            for swipe in pointer.swipes.for_client(client) {
                swipe.end(serial, millis(time), i32::from(cancelled));
            }
            true
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
            false
        }
        (Gesture::PinchEnd { cancelled, time }, GestureKind::Pinch) => {
            for pinch in pointer.pinches.for_client(client) {
                pinch.end(serial, millis(time), i32::from(cancelled));
            }
            true
        }
        (Gesture::HoldEnd { cancelled, time }, GestureKind::Hold) => {
            for hold in pointer.holds.for_client(client) {
                hold.end(serial, millis(time), i32::from(cancelled));
            }
            true
        }
        _ => return,
    };
    if ended && let Some(entry) = seats.seat_mut(seat) {
        entry.pointer.gesture = None;
    }
    mark(seats, seat, wire, client);
}

fn begin_gesture<A: Access>(
    state: &mut A,
    seat: SeatId,
    kind: GestureKind,
    fingers: u32,
    time: u32,
    serial: u32,
) {
    let Some(client) = state
        .seats()
        .get(seat)
        .and_then(|entry| focused_client(&entry.pointer))
    else {
        return;
    };
    if !carry::claim(state, seat, client, Device::Pointer, Claim::Active) {
        return;
    }
    let seats = state.seats();
    let wire = seats.wire(seat, client);
    let (Some(entry), Some(wired)) = (seats.get(seat), seats.get(wire)) else {
        return;
    };
    let Some(focused) = &entry.pointer.focused else {
        return;
    };
    let surface = focused.focus.surface;
    let wl = &focused.surface;
    let pointer = &wired.pointer;
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
    seats.carry_serial(seat, wire, serial);
    let Some(entry) = seats.seat_mut(seat) else {
        return;
    };
    entry.pointer.gesture = Some(ActiveGesture {
        kind,
        surface,
        client,
    });
    entry.pointer.mark(wire, client);
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
    if pointer.focused_surface() != Some(surface) {
        return;
    }
    let client = pointer.focused.take().map(|focused| focused.client());
    if let Some(client) = client {
        carry::vacate(state, seat, client, Device::Pointer);
        deliver_frame(state, seat);
    }
    show(
        state,
        seat,
        CursorOwner::Pointer,
        CursorImage::default(),
        None,
    );
}

pub(crate) fn introduce<A: Access>(state: &mut A, seat: SeatId, resource: WlPointer) {
    let client = resource.client();
    let seats = state.seats();
    let Some(entry) = seats.seat_mut(seat) else {
        return;
    };
    entry.pointer.pointers.add(resource.clone());
    let presenter = seats.presenter(seat, client, Device::Pointer);
    let Some(entry) = seats.get(presenter) else {
        return;
    };
    let pointer = &entry.pointer;
    let Some(focused) = pointer
        .focused
        .as_ref()
        .filter(|focused| focused.client() == client)
    else {
        return;
    };
    let local = focused.focus.local(pointer.location);
    resource.enter(pointer.enter_serial, &focused.surface, local.x, local.y);
    resource.frame();
}

fn pointers(seats: &Seats, wire: SeatId, client: ClientId) -> &[WlPointer] {
    seats
        .get(wire)
        .map_or(&[], |entry| entry.pointer.pointers.for_client(client))
}

fn mark(seats: &mut Seats, seat: SeatId, wire: SeatId, client: ClientId) {
    if let Some(entry) = seats.seat_mut(seat) {
        entry.pointer.mark(wire, client);
    }
}

fn focused_client(pointer: &Pointer) -> Option<ClientId> {
    pointer.focused.as_ref().map(Focused::client)
}

fn locked(pointer: &Pointer) -> bool {
    pointer
        .constraint()
        .is_some_and(|(_, kind)| kind == constraint::ConstraintKind::Lock)
}
