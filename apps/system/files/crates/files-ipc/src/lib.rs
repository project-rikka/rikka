mod request;
mod service;

pub use request::{Request, Reveal, Sink};
pub use service::{Service, start};

pub const DESKTOP_NAME: &str = "org.freedesktop.FileManager1";

pub const DESKTOP_PATH: &str = "/org/freedesktop/FileManager1";
