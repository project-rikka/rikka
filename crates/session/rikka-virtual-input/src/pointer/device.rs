use rikka_wayland::{Clock, Host};
use rikka_wayland_client::wl_pointer::{Axis, AxisSource, ButtonState};
use rikka_wayland_client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use rikka_wayland_client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;

use crate::absolute_position::AbsolutePosition;
use crate::error::Result;
use crate::pointer::detent::{Detent, NOTCH};
use crate::pointer::output::NamedOutput;
use crate::pointer::queue::{Command, Queue};
use crate::seat::SeatHandle;

const PIXELS_PER_NOTCH: f64 = 15.0;
const WITH_OUTPUT_SINCE: u32 = 2;

pub(super) struct Device {
    host: Host,
    pointer: ZwlrVirtualPointerV1,
    output: Option<NamedOutput>,
    clock: Clock,
    pressed: Vec<u32>,
    horizontal: Detent,
    vertical: Detent,
}

impl Device {
    pub(super) fn open(seat: &SeatHandle, output: Option<&str>) -> Result<Device> {
        let display = seat.host.display();
        let (pointer, output) = match output {
            None => {
                let manager: ZwlrVirtualPointerManagerV1 = display.bind(1..=WITH_OUTPUT_SINCE)?;
                let pointer = manager.create_virtual_pointer(Some(&seat.seat));
                (pointer, None)
            }
            Some(name) => {
                let manager: ZwlrVirtualPointerManagerV1 =
                    display.bind(WITH_OUTPUT_SINCE..=WITH_OUTPUT_SINCE)?;
                let output = NamedOutput::find(display, name)?;
                let pointer = manager
                    .create_virtual_pointer_with_output(Some(&seat.seat), Some(output.output()));
                (pointer, Some(output))
            }
        };
        Ok(Device {
            host: seat.host.clone(),
            pointer,
            output,
            clock: Clock::new(),
            pressed: Vec::new(),
            horizontal: Detent::default(),
            vertical: Detent::default(),
        })
    }

    pub(super) fn run(&mut self, queue: &Queue) {
        while let Some(commands) = queue.take() {
            for command in commands {
                match command {
                    Command::MoveTo(position) => self.move_to(position),
                    Command::MoveBy { dx, dy } => self.move_by(dx, dy),
                    Command::Button { code, pressed } => self.button(code, pressed),
                    Command::Wheel { x120, y120 } => self.wheel(x120, y120),
                    Command::Continuous { dx, dy } => self.continuous(dx, dy),
                    Command::Sync(done) => {
                        self.flush();
                        if done.send(()).is_err() {
                            tracing::trace!("pointer sync waiter is gone");
                        }
                    }
                }
            }
            self.flush();
        }
        self.close();
    }

    fn move_to(&mut self, position: AbsolutePosition) {
        let time = self.clock.now();
        self.pointer.motion_absolute(
            time,
            position.x,
            position.y,
            position.x_extent,
            position.y_extent,
        );
        self.pointer.frame();
    }

    fn move_by(&mut self, dx: f64, dy: f64) {
        let time = self.clock.now();
        self.pointer.motion(time, dx, dy);
        self.pointer.frame();
    }

    fn button(&mut self, code: u32, pressed: bool) {
        let held = self.pressed.iter().position(|button| *button == code);
        let state = match (pressed, held) {
            (true, None) => {
                self.pressed.push(code);
                ButtonState::Pressed
            }
            (false, Some(index)) => {
                self.pressed.remove(index);
                ButtonState::Released
            }
            (true, Some(_)) | (false, None) => return,
        };
        let time = self.clock.now();
        self.pointer.button(time, code, state);
        self.pointer.frame();
    }

    fn wheel(&mut self, x120: i32, y120: i32) {
        if x120 == 0 && y120 == 0 {
            return;
        }
        let time = self.clock.now();
        self.pointer.axis_source(AxisSource::Wheel);
        for (axis, value120, detent) in [
            (Axis::HorizontalScroll, x120, &mut self.horizontal),
            (Axis::VerticalScroll, y120, &mut self.vertical),
        ] {
            if value120 == 0 {
                continue;
            }
            let value = f64::from(value120) * PIXELS_PER_NOTCH / f64::from(NOTCH);
            match detent.turn(value120) {
                0 => self.pointer.axis(time, axis, value),
                notches => self.pointer.axis_discrete(time, axis, value, notches),
            }
        }
        self.pointer.frame();
    }

    fn continuous(&mut self, dx: f64, dy: f64) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let time = self.clock.now();
        self.pointer.axis_source(AxisSource::Continuous);
        for (axis, value) in [(Axis::HorizontalScroll, dx), (Axis::VerticalScroll, dy)] {
            if value != 0.0 {
                self.pointer.axis(time, axis, value);
            }
        }
        self.pointer.frame();
    }

    fn flush(&mut self) {
        if let Some(output) = &mut self.output
            && let Err(error) = output.dispatch()
        {
            tracing::debug!(%error, "failed to dispatch virtual pointer output events");
        }
        if let Err(error) = self.host.display().flush() {
            tracing::warn!(%error, "failed to flush virtual pointer");
        }
    }

    fn close(&mut self) {
        let pressed = std::mem::take(&mut self.pressed);
        let released = !pressed.is_empty();
        for code in pressed.into_iter().rev() {
            let time = self.clock.now();
            self.pointer.button(time, code, ButtonState::Released);
        }
        if released {
            self.pointer.frame();
        }
        self.pointer.destroy();
        if let Some(output) = &self.output {
            output.release();
        }
        let display = self.host.display();
        let delivered = match released {
            true => display.event_loop().settle(&mut ()),
            false => display.flush(),
        };
        if let Err(error) = delivered {
            tracing::debug!(%error, "failed to flush virtual pointer teardown");
        }
    }
}
