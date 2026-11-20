mod action;
mod command;
mod device_event;
mod error;
mod layouts;
mod single;

pub mod cursor;
pub mod evdev;
pub mod win32;
pub mod xkb;

pub use action::{ActionMap, Binding, Chord, parse_modifiers};
pub use command::Command;
pub use device_event::{DeviceEvent, ScrollPhase, ScrollSource};
pub use error::{Error, Result};
pub use layouts::{Layout, Layouts, Variant};

#[cfg(doc)]
use rikka_types::{Key, Modifiers, PointerButton};
