use input::event::touch::{TouchEvent, TouchEventPosition, TouchEventSlot, TouchEventTrait};
use rikka_compositor_geometry::Point;

use crate::device::DeviceId;
use crate::event::InputEvent;
use rikka_compositor_reactor::Timestamp;

pub(super) fn translate(event: TouchEvent, device: DeviceId) -> Option<InputEvent> {
    let time = Timestamp::from_micros(event.time_usec());
    match event {
        TouchEvent::Down(down) => Some(InputEvent::TouchDown {
            device,
            time,
            slot: down.seat_slot(),
            position: Point::new(down.x_transformed(1), down.y_transformed(1)),
        }),
        TouchEvent::Motion(motion) => Some(InputEvent::TouchMotion {
            device,
            time,
            slot: motion.seat_slot(),
            position: Point::new(motion.x_transformed(1), motion.y_transformed(1)),
        }),
        TouchEvent::Up(up) => Some(InputEvent::TouchUp {
            device,
            time,
            slot: up.seat_slot(),
        }),
        TouchEvent::Cancel(_) => Some(InputEvent::TouchCancel { device, time }),
        TouchEvent::Frame(_) => Some(InputEvent::TouchFrame { device, time }),
        _ => None,
    }
}
