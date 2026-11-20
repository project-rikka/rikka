use input::event::pointer::{Axis, PointerEvent, PointerEventTrait, PointerScrollEvent};
use rikka_compositor_geometry::Point;
use rikka_compositor_reactor::Timestamp;
use rikka_compositor_types::AxisSource;

use crate::device::DeviceId;
use crate::event::InputEvent;
use crate::scroll::ScrollAxis;

pub(super) fn translate(
    event: PointerEvent,
    device: DeviceId,
    natural: bool,
) -> Option<InputEvent> {
    let time = Timestamp::from_micros(event.time_usec());
    let at = |source| Stamp {
        device,
        time,
        source,
        natural,
    };
    match event {
        PointerEvent::Motion(motion) => Some(InputEvent::PointerMotion {
            device,
            time,
            delta: Point::new(motion.dx(), motion.dy()),
            unaccelerated: Point::new(motion.dx_unaccelerated(), motion.dy_unaccelerated()),
            moved_to: None,
        }),
        PointerEvent::MotionAbsolute(motion) => Some(InputEvent::PointerMotionAbsolute {
            device,
            time,
            position: Point::new(
                motion.absolute_x_transformed(1),
                motion.absolute_y_transformed(1),
            ),
        }),
        PointerEvent::Button(button) => Some(InputEvent::PointerButton {
            device,
            time,
            code: button.button(),
            press: super::press(button.button_state()),
            seat_count: button.seat_button_count(),
        }),
        PointerEvent::ScrollWheel(wheel) => Some(scroll(&wheel, at(AxisSource::Wheel), |axis| {
            wheel.scroll_value_v120(axis)
        })),
        PointerEvent::ScrollFinger(finger) => {
            Some(scroll(&finger, at(AxisSource::Finger), |_| 0.0))
        }
        PointerEvent::ScrollContinuous(continuous) => {
            Some(scroll(&continuous, at(AxisSource::Continuous), |_| 0.0))
        }
        _ => None,
    }
}

struct Stamp {
    device: DeviceId,
    time: Timestamp,
    source: AxisSource,
    natural: bool,
}

fn scroll(event: &impl PointerScrollEvent, stamp: Stamp, v120: impl Fn(Axis) -> f64) -> InputEvent {
    let axis = |axis: Axis| {
        let present = event.has_axis(axis);
        let (value, v120) = match present {
            true => (event.scroll_value(axis), v120(axis)),
            false => (0.0, 0.0),
        };
        ScrollAxis::read(stamp.source, present, value, v120)
    };
    InputEvent::Scroll {
        device: stamp.device,
        time: stamp.time,
        source: stamp.source,
        horizontal: axis(Axis::Horizontal),
        vertical: axis(Axis::Vertical),
        natural: stamp.natural,
    }
}
