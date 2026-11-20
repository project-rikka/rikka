use std::time::Duration;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::wl_pointer::{
    Axis as WireAxis, AxisSource as WireSource, ButtonState,
};
use rikka_compositor_protocols::wl_seat::WlSeat;
use rikka_compositor_protocols::zwlr_virtual_pointer_manager_v1::{
    self as manager, ZwlrVirtualPointerManagerV1,
};
use rikka_compositor_protocols::zwlr_virtual_pointer_v1::{self as protocol, ZwlrVirtualPointerV1};
use rikka_compositor_types::{AxisSource, Press};

use crate::access::Access;
use crate::event::SeatEvent;
use crate::ids::{SeatId, VirtualPointerId};
use crate::pointer::{Axis, AxisMotion, Button};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VirtualPointerInput {
    Motion {
        delta: Point<f64, Logical>,
        time: Duration,
    },
    MotionAbsolute {
        fraction: Point<f64, Logical>,
        time: Duration,
    },
    Button(Button),
    Axis(Axis),
}

#[derive(Debug)]
pub(crate) struct VirtualPointer {
    seat: SeatId,
    output: Option<WlOutput>,
    axis: Option<Axis>,
}

pub(crate) fn virtual_pointer_manager<A: Access>(
    state: &mut A,
    _manager: ZwlrVirtualPointerManagerV1,
    request: manager::Request,
) {
    let (seat, output, id) = match request {
        manager::Request::CreateVirtualPointer { seat, id } => (seat, None, id),
        manager::Request::CreateVirtualPointerWithOutput { seat, output, id } => (seat, output, id),
        manager::Request::Destroy => return,
    };
    let Some(seat) = resolve(state, seat.as_ref()) else {
        return;
    };
    let pointer = state.seats().virtual_pointers.insert(VirtualPointer {
        seat,
        output,
        axis: None,
    });
    id.set_key(pointer);
}

pub(crate) fn virtual_pointer_destroyed<A: Access>(state: &mut A, pointer: ZwlrVirtualPointerV1) {
    if let Some(key) = pointer.key::<VirtualPointerId>() {
        state.seats().virtual_pointers.remove(key);
    }
}

pub(crate) fn virtual_pointer_request<A: Access>(
    state: &mut A,
    pointer: ZwlrVirtualPointerV1,
    request: protocol::Request,
) {
    let Some(key) = pointer.key::<VirtualPointerId>() else {
        return;
    };
    let input = match request {
        protocol::Request::Destroy => return,
        protocol::Request::Motion { time, dx, dy } => VirtualPointerInput::Motion {
            delta: Point::new(dx, dy),
            time: millis(time),
        },
        protocol::Request::MotionAbsolute {
            time,
            x,
            y,
            x_extent,
            y_extent,
        } => {
            if x_extent == 0 || y_extent == 0 {
                return;
            }
            VirtualPointerInput::MotionAbsolute {
                fraction: Point::new(
                    f64::from(x) / f64::from(x_extent),
                    f64::from(y) / f64::from(y_extent),
                ),
                time: millis(time),
            }
        }
        protocol::Request::Button {
            time,
            button,
            state: pressed,
        } => {
            let press = match pressed {
                Ok(ButtonState::Pressed) => Press::Pressed,
                Ok(ButtonState::Released) => Press::Released,
                Err(_) => return,
            };
            VirtualPointerInput::Button(Button {
                code: button,
                press,
                time: millis(time),
            })
        }
        protocol::Request::Axis { time, axis, value } => {
            if let Some(orientation) = axis_of(&pointer, axis) {
                stage(state, key, time, |axis| {
                    orientation.motion_mut(axis).value += value
                });
            }
            return;
        }
        protocol::Request::AxisDiscrete {
            time,
            axis,
            value,
            discrete,
        } => {
            if let Some(orientation) = axis_of(&pointer, axis) {
                stage(state, key, time, |axis| {
                    let motion = orientation.motion_mut(axis);
                    motion.value += value;
                    motion.v120 = Some(motion.v120.unwrap_or(0) + discrete.saturating_mul(120));
                });
            }
            return;
        }
        protocol::Request::AxisStop { time, axis } => {
            if let Some(orientation) = axis_of(&pointer, axis) {
                stage(state, key, time, |axis| {
                    orientation.motion_mut(axis).stopped = true
                });
            }
            return;
        }
        protocol::Request::AxisSource { axis_source } => {
            let source = match axis_source {
                Ok(WireSource::Wheel) => AxisSource::Wheel,
                Ok(WireSource::Finger) => AxisSource::Finger,
                Ok(WireSource::Continuous) => AxisSource::Continuous,
                Ok(WireSource::WheelTilt) => AxisSource::WheelTilt,
                Err(_) => {
                    pointer.post_error(protocol::Error::InvalidAxisSource);
                    return;
                }
            };
            stage(state, key, 0, |axis| axis.source = Some(source));
            return;
        }
        protocol::Request::Frame => {
            let Some(axis) = state
                .seats()
                .virtual_pointers
                .get_mut(key)
                .and_then(|virtual_pointer| virtual_pointer.axis.take())
            else {
                return;
            };
            VirtualPointerInput::Axis(axis)
        }
    };
    let Some((seat, output)) = state
        .seats()
        .virtual_pointers
        .get(key)
        .map(|virtual_pointer| (virtual_pointer.seat, virtual_pointer.output.clone()))
    else {
        return;
    };
    state.report(SeatEvent::VirtualPointer {
        seat,
        output,
        input,
    });
}

#[derive(Debug, Clone, Copy)]
enum Orientation {
    Horizontal,
    Vertical,
}

fn axis_of(
    pointer: &ZwlrVirtualPointerV1,
    axis: Result<WireAxis, rikka_wayland_wire::UnknownValue>,
) -> Option<Orientation> {
    match axis {
        Ok(WireAxis::HorizontalScroll) => Some(Orientation::Horizontal),
        Ok(WireAxis::VerticalScroll) => Some(Orientation::Vertical),
        Err(_) => {
            pointer.post_error(protocol::Error::InvalidAxis);
            None
        }
    }
}

impl Orientation {
    fn motion_mut(self, axis: &mut Axis) -> &mut AxisMotion {
        match self {
            Self::Horizontal => &mut axis.horizontal,
            Self::Vertical => &mut axis.vertical,
        }
    }
}

fn stage<A: Access>(
    state: &mut A,
    key: VirtualPointerId,
    time: u32,
    visit: impl FnOnce(&mut Axis),
) {
    let Some(pointer) = state.seats().virtual_pointers.get_mut(key) else {
        return;
    };
    let axis = pointer.axis.get_or_insert_with(|| Axis::new(millis(time)));
    if time != 0 {
        axis.time = millis(time);
    }
    visit(axis);
}

fn resolve<A: Access>(state: &mut A, seat: Option<&WlSeat>) -> Option<SeatId> {
    match seat {
        Some(seat) => seat.key::<SeatId>(),
        None => state.seats().primary(),
    }
}

fn millis(time: u32) -> Duration {
    Duration::from_millis(u64::from(time))
}
