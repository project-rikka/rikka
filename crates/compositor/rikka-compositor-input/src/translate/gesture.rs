use input::event::gesture::{
    GestureEndEvent, GestureEvent, GestureEventCoordinates, GestureEventTrait, GestureHoldEvent,
    GesturePinchEvent, GesturePinchEventTrait, GestureSwipeEvent,
};
use rikka_compositor_geometry::Point;
use rikka_compositor_reactor::Timestamp;
use rikka_compositor_types::Gesture;

use crate::device::DeviceId;
use crate::event::InputEvent;

pub(super) fn translate(event: GestureEvent, device: DeviceId) -> Option<InputEvent> {
    let time = Timestamp::from_micros(event.time_usec()).as_monotonic();
    let fingers = u32::try_from(event.finger_count()).unwrap_or(0);
    let gesture = match event {
        GestureEvent::Swipe(GestureSwipeEvent::Begin(_)) => Gesture::SwipeBegin { fingers, time },
        GestureEvent::Swipe(GestureSwipeEvent::Update(update)) => Gesture::SwipeUpdate {
            delta: Point::new(update.dx(), update.dy()),
            time,
        },
        GestureEvent::Swipe(GestureSwipeEvent::End(end)) => Gesture::SwipeEnd {
            cancelled: end.cancelled(),
            time,
        },
        GestureEvent::Pinch(GesturePinchEvent::Begin(_)) => Gesture::PinchBegin { fingers, time },
        GestureEvent::Pinch(GesturePinchEvent::Update(update)) => Gesture::PinchUpdate {
            delta: Point::new(update.dx(), update.dy()),
            scale: update.scale(),
            rotation: update.angle_delta(),
            time,
        },
        GestureEvent::Pinch(GesturePinchEvent::End(end)) => Gesture::PinchEnd {
            cancelled: end.cancelled(),
            time,
        },
        GestureEvent::Hold(GestureHoldEvent::Begin(_)) => Gesture::HoldBegin { fingers, time },
        GestureEvent::Hold(GestureHoldEvent::End(end)) => Gesture::HoldEnd {
            cancelled: end.cancelled(),
            time,
        },
        _ => return None,
    };
    Some(InputEvent::Gesture { device, gesture })
}
