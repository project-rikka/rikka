mod connect;
mod globals;

include!("protocols.rs");

pub use connect::{connect_to, connect_to_env, from_socket};
pub use globals::{Global, Globals};
pub use rikka_wayland_wire::client::{
    Barrier, Connection, Error, Handle, ObjectId, ProtocolError, Proxy, Queue, Result, Route,
};
pub use rikka_wayland_wire::{Key, UnknownValue, raw_value, serial_not_before, slot_keys};
