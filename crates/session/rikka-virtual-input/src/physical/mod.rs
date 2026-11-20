mod device;

use rikka_input::evdev;
use rikka_types::KeyCode;
use rikka_worker::{CommandReceiver, CommandSender, SendError, Worker, command_channel};

use crate::error::{Error, Result, from_worker};
use crate::seat::SeatHandle;

use device::Device;

pub(crate) enum Command {
    Press(u32),
    Release(u32),
    ReleaseAll,
    Stop,
}

#[derive(Debug)]
pub struct PhysicalKeyboard {
    worker: Worker<CommandSender<Command>>,
}

impl PhysicalKeyboard {
    pub(crate) fn new(seat: SeatHandle) -> Result<PhysicalKeyboard> {
        let (commands, receiver): (_, CommandReceiver<Command>) =
            command_channel(|| Command::Stop)?;
        let worker = Worker::spawn("rikka-physical", commands, move || {
            let mut device = Device::open(&seat)?;
            Ok(move || device.run(&receiver))
        })
        .map_err(from_worker)?;
        Ok(PhysicalKeyboard { worker })
    }

    pub fn press(&self, code: KeyCode) -> Result<()> {
        self.send(Command::Press(evdev_code(code)?))
    }

    pub fn release(&self, code: KeyCode) -> Result<()> {
        self.send(Command::Release(evdev_code(code)?))
    }

    pub fn release_all(&self) -> Result<()> {
        self.send(Command::ReleaseAll)
    }

    fn send(&self, command: Command) -> Result<()> {
        self.worker
            .handle()
            .send(command)
            .map_err(|error| match error {
                SendError::Io(error) => Error::Io(error),
                SendError::Disconnected | SendError::Timeout => Error::Stopped,
            })
    }
}

fn evdev_code(code: KeyCode) -> Result<u32> {
    match code.is_known() {
        true => Ok(code.xkb() - evdev::XKB_OFFSET),
        false => Err(Error::UnknownKeyCode),
    }
}
