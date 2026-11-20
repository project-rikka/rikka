use rikka_compositor_geometry::Point;
use rikka_compositor_seat::InputDevice;
use rikka_compositor_types::Press;

use super::pending::{AbsoluteSource, PendingEvent, Spot, Touched};
use crate::render::monotonic;
use crate::state::Compositor;

const INJECTED: InputDevice = InputDevice(0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Injected {
    PointerTo { x: f64, y: f64 },
    Button { code: u32, pressed: bool },
    Key { code: u32, pressed: bool },
    TouchDown { slot: u32, x: f64, y: f64 },
    TouchUp { slot: u32 },
    TouchFrame,
}

fn press(pressed: bool) -> Press {
    match pressed {
        true => Press::Pressed,
        false => Press::Released,
    }
}

impl Compositor {
    pub fn inject(&mut self, injected: Injected) {
        let time = monotonic();
        let event = match injected {
            Injected::PointerTo { x, y } => {
                let to = Point::new(x, y);
                self.input
                    .pending
                    .push_pointed(AbsoluteSource::Injected, to, time);
                self.frames.input_arrived();
                return;
            }
            Injected::Button { code, pressed } => PendingEvent::Button {
                code,
                press: press(pressed),
                time,
            },
            Injected::Key { code, pressed } => PendingEvent::Key {
                device: INJECTED,
                code,
                press: press(pressed),
                time,
            },
            Injected::TouchDown { slot, x, y } => PendingEvent::Touch {
                touched: Touched::Down {
                    slot,
                    spot: Spot::At(Point::new(x, y)),
                },
                time,
            },
            Injected::TouchUp { slot } => PendingEvent::Touch {
                touched: Touched::Up { slot },
                time,
            },
            Injected::TouchFrame => PendingEvent::Touch {
                touched: Touched::Frame,
                time,
            },
        };
        self.input.pending.push(event);
        self.frames.input_arrived();
    }
}
