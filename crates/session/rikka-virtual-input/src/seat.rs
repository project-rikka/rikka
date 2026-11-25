use std::path::Path;

use rikka_wayland::{Display, Host};
use rikka_wayland_client::wl_seat::WlSeat;

use crate::cap::Cap;
use crate::error::Result;
use crate::keys::{Keyboard, TextKeyboard};
use crate::physical::PhysicalKeyboard;
use crate::pointer::Pointer;

#[derive(Debug)]
pub struct Seat {
    host: Host,
    seat: WlSeat,
}

impl Seat {
    pub fn connect() -> Result<Seat> {
        Seat::new(Display::from_env()?, None)
    }

    pub fn connect_to(socket: &Path) -> Result<Seat> {
        Seat::new(Display::connect_to(socket)?, None)
    }

    pub fn connect_named(socket: &Path, seat: &str) -> Result<Seat> {
        Seat::new(Display::connect_to(socket)?, Some(seat))
    }

    fn new(display: Display, name: Option<&str>) -> Result<Seat> {
        let seat = display.seat(name)?;
        let host = Host::with_reader(display)?;
        Ok(Seat { host, seat })
    }

    pub fn pointer(&self) -> Result<Pointer> {
        Pointer::new(self.handle(), None)
    }

    pub fn pointer_on_output(&self, output: &str) -> Result<Pointer> {
        Pointer::new(self.handle(), Some(output.to_owned()))
    }

    pub fn text_keyboard(&self) -> Result<TextKeyboard> {
        TextKeyboard::new(self.handle())
    }

    pub fn keyboard(&self, caps: impl IntoIterator<Item = Cap>) -> Result<Keyboard> {
        Keyboard::new(self.handle(), caps)
    }

    pub fn physical_keyboard(&self) -> Result<PhysicalKeyboard> {
        PhysicalKeyboard::new(self.handle())
    }

    fn handle(&self) -> SeatHandle {
        SeatHandle {
            host: self.host.clone(),
            seat: self.seat.clone(),
        }
    }
}

pub(crate) struct SeatHandle {
    pub(crate) host: Host,
    pub(crate) seat: WlSeat,
}
