use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn input thread")]
    Spawn(#[source] io::Error),
    #[error("libinput failed to assign seat `{0}`")]
    Seat(String),
    #[error("input thread event loop failed")]
    Reactor(#[source] io::Error),
    #[error("input thread has stopped")]
    Stopped,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
