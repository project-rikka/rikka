use std::os::fd::OwnedFd;
use std::path::Path;

use rikka_wayland_wire::client::{Connection, Handle, Proxy, Result};

use crate::wl_display::{INTERFACE, WlDisplay};

pub fn connect_to_env() -> Result<(Connection, WlDisplay)> {
    Connection::connect_to_env(&INTERFACE).map(typed)
}

pub fn connect_to(path: &Path) -> Result<(Connection, WlDisplay)> {
    Connection::connect_to(path, &INTERFACE).map(typed)
}

pub fn from_socket(socket: impl Into<OwnedFd>) -> Result<(Connection, WlDisplay)> {
    Connection::from_socket(socket, &INTERFACE).map(typed)
}

fn typed((connection, display): (Connection, Handle)) -> (Connection, WlDisplay) {
    let display = WlDisplay::from_handle(display).expect("the display handle is a wl_display");
    (connection, display)
}
