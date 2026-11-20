mod detent;
mod device;
mod output;
mod queue;

use std::sync::Arc;

use rikka_input::evdev::button_code;
use rikka_types::PointerButton;

use crate::absolute_position::AbsolutePosition;
use crate::error::{Error, Result, from_worker};
use crate::seat::SeatHandle;

use detent::NOTCH;
use device::Device;
use queue::{Command, Queue};

#[derive(Debug)]
pub struct Pointer {
    worker: rikka_worker::Worker<Arc<Queue>>,
}

impl Pointer {
    pub(crate) fn new(seat: SeatHandle, output: Option<String>) -> Result<Pointer> {
        let queue = Arc::new(Queue::default());
        let worker_queue = Arc::clone(&queue);

        let worker = rikka_worker::Worker::spawn("rikka-pointer", queue, move || {
            let mut device = Device::open(&seat, output.as_deref())?;
            Ok(move || device.run(&worker_queue))
        })
        .map_err(from_worker)?;
        Ok(Pointer { worker })
    }

    pub fn move_to(&self, position: AbsolutePosition) -> Result<()> {
        if !position.is_in_bounds() {
            return Err(Error::OutOfBounds(position));
        }
        self.push(Command::MoveTo(position))
    }

    pub fn move_by(&self, dx: f64, dy: f64) -> Result<()> {
        if dx == 0.0 && dy == 0.0 {
            return Ok(());
        }
        self.push(Command::MoveBy { dx, dy })
    }

    pub fn press(&self, button: PointerButton) -> Result<()> {
        self.push(Command::Button {
            code: button_code(button),
            pressed: true,
        })
    }

    pub fn release(&self, button: PointerButton) -> Result<()> {
        self.push(Command::Button {
            code: button_code(button),
            pressed: false,
        })
    }

    pub fn scroll(&self, dx: f64, dy: f64) -> Result<()> {
        self.scroll_value120(value120(dx), value120(dy))
    }

    pub fn scroll_value120(&self, x120: i32, y120: i32) -> Result<()> {
        if x120 == 0 && y120 == 0 {
            return Ok(());
        }
        self.push(Command::Wheel { x120, y120 })
    }

    pub fn scroll_continuous(&self, dx: f64, dy: f64) -> Result<()> {
        if dx == 0.0 && dy == 0.0 {
            return Ok(());
        }
        self.push(Command::Continuous { dx, dy })
    }

    pub fn sync(&self) -> Result<()> {
        self.worker.handle().sync()
    }

    fn push(&self, command: Command) -> Result<()> {
        self.worker.handle().push(command)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a scroll of more than i32::MAX / 120 notches saturates"
)]
fn value120(notches: f64) -> i32 {
    (notches * f64::from(NOTCH)).round() as i32
}
