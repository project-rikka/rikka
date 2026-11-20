use rikka_compositor_protocols::zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1;
use rikka_compositor_protocols::zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1;
use rikka_compositor_protocols::zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1;
use rikka_compositor_protocols::zwp_pointer_gestures_v1::{self as manager, ZwpPointerGesturesV1};
use rikka_wayland_wire::Resource;

use crate::access::Access;
use crate::host::SeatHost;
use crate::ids::SeatId;

pub(crate) fn gestures_manager<A: Access>(
    state: &mut A,
    _manager: ZwpPointerGesturesV1,
    request: manager::Request,
) {
    match request {
        manager::Request::GetSwipeGesture { id, pointer } => {
            let Some(seat) = pointer.key::<SeatId>() else {
                return;
            };
            id.set_key(seat);
            if let Some(entry) = state.seats().seat_mut(seat) {
                entry.pointer.swipes.add(id);
            }
        }
        manager::Request::GetPinchGesture { id, pointer } => {
            let Some(seat) = pointer.key::<SeatId>() else {
                return;
            };
            id.set_key(seat);
            if let Some(entry) = state.seats().seat_mut(seat) {
                entry.pointer.pinches.add(id);
            }
        }
        manager::Request::GetHoldGesture { id, pointer } => {
            let Some(seat) = pointer.key::<SeatId>() else {
                return;
            };
            id.set_key(seat);
            if let Some(entry) = state.seats().seat_mut(seat) {
                entry.pointer.holds.add(id);
            }
        }
        manager::Request::Release => {}
    }
}

pub(crate) fn gesture_request<S: SeatHost, R: Resource>(
    _state: &mut S,
    _gesture: R,
    _request: R::Request<'_>,
) {
}

pub(crate) fn gesture_destroyed<S: SeatHost, R: Resource>(state: &mut S, gesture: R) {
    let object = gesture.object();
    let Some(seat) = object.key::<SeatId>() else {
        return;
    };
    let Some(entry) = state.seats().seat_mut(seat) else {
        return;
    };
    let pointer = &mut entry.pointer;
    if let Some(swipe) = ZwpPointerGestureSwipeV1::from_object(object.clone()) {
        pointer.swipes.remove(&swipe);
    } else if let Some(pinch) = ZwpPointerGesturePinchV1::from_object(object.clone()) {
        pointer.pinches.remove(&pinch);
    } else if let Some(hold) = ZwpPointerGestureHoldV1::from_object(object.clone()) {
        pointer.holds.remove(&hold);
    }
}
