mod error;
mod launch_request;
mod name;
mod service;

pub use error::{Error, Result};
pub use launch_request::{LaunchRequest, Sink};
pub use name::Name;
pub use service::{Peer, Service, Startup, claim};

pub const TAG_ENV: &str = "RIKKA_APP_INSTANCE";
