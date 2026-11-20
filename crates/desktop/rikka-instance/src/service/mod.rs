#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{Peer, Service, claim};
#[cfg(windows)]
pub use windows::{Peer, Service, claim};

#[derive(Debug)]
pub enum Startup {
    Primary(Service),
    Running(Peer),
    Standalone,
}
