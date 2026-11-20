use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;
use smallvec::SmallVec;

use crate::access::Access;
use crate::ids::SeatId;
use crate::input::millis;
use crate::surface;
use crate::touch::TouchPoint;
use crate::touch::events::{TouchDown, TouchMotion, TouchShape, TouchUp};

pub(crate) fn touch_down<A: Access>(state: &mut A, seat: SeatId, down: &TouchDown) {
    let Some(focus) = down.under else {
        return;
    };
    let Some(wl) = surface::resource(state, focus.surface) else {
        return;
    };
    let serial = state.display().next_serial();
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let touch = &mut entry.touch;
    touch.points.retain(|point| point.slot != down.slot);
    let client = wl.client();
    touch.points.push(TouchPoint {
        slot: down.slot,
        focus,
        client,
        serial,
        location: down.location,
    });
    let local = focus.local(down.location);
    for resource in touch.touches.for_client(client) {
        resource.down(
            serial,
            millis(down.time),
            &wl,
            down.slot as i32,
            local.x,
            local.y,
        );
    }
    touch.mark(client);
}

pub(crate) fn touch_motion<A: Access>(state: &mut A, seat: SeatId, motion: &TouchMotion) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let touch = &mut entry.touch;
    let Some(point) = touch
        .points
        .iter_mut()
        .find(|point| point.slot == motion.slot)
    else {
        return;
    };
    point.location = motion.location;
    let point = *point;
    let local = point.focus.local(motion.location);
    for resource in touch.touches.for_client(point.client) {
        resource.motion(millis(motion.time), motion.slot as i32, local.x, local.y);
    }
    touch.mark(point.client);
}

pub(crate) fn touch_up<A: Access>(state: &mut A, seat: SeatId, up: &TouchUp) {
    let serial = state.display().next_serial();
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let touch = &mut entry.touch;
    let Some(index) = touch.points.iter().position(|point| point.slot == up.slot) else {
        return;
    };
    let point = touch.points.remove(index);
    for resource in touch.touches.for_client(point.client) {
        resource.up(serial, millis(up.time), up.slot as i32);
    }
    touch.mark(point.client);
}

pub(crate) fn touch_shape<A: Access>(state: &mut A, seat: SeatId, shape: &TouchShape) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let touch = &mut entry.touch;
    let Some(point) = touch
        .points
        .iter()
        .find(|point| point.slot == shape.slot)
        .copied()
    else {
        return;
    };
    for resource in touch.touches.for_client(point.client) {
        resource.shape(shape.slot as i32, shape.major, shape.minor);
        if let Some(orientation) = shape.orientation {
            resource.orientation(shape.slot as i32, orientation);
        }
    }
    touch.mark(point.client);
}

pub(crate) fn touch_frame<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let touch = &mut entry.touch;
    for client in touch.unframed.drain(..) {
        for resource in touch.touches.for_client(client) {
            resource.frame();
        }
    }
}

pub(crate) fn touch_cancel<A: Access>(state: &mut A, seat: SeatId) {
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let touch = &mut entry.touch;
    let mut clients: SmallVec<[ClientId; 2]> = SmallVec::new();
    for point in touch.points.drain(..) {
        if !clients.contains(&point.client) {
            clients.push(point.client);
        }
    }
    for client in clients {
        for resource in touch.touches.for_client(client) {
            resource.cancel();
        }
    }
    touch.unframed.clear();
}

pub(crate) fn touch_surface_gone<A: Access>(state: &mut A, seat: SeatId, surface: SurfaceId) {
    if let Some(entry) = state.seats().seat_mut(seat) {
        entry
            .touch
            .points
            .retain(|point| point.focus.surface != surface);
    }
}
