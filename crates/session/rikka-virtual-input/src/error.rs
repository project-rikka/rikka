use rikka_types::Key;

use crate::absolute_position::AbsolutePosition;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no usable Wayland seat: {0}")]
    Wayland(#[from] rikka_wayland::Error),

    #[error("input worker has stopped")]
    Stopped,

    #[error(
        "({}, {}) is outside the {}x{} area",
        .0.x, .0.y, .0.x_extent, .0.y_extent
    )]
    OutOfBounds(AbsolutePosition),

    #[error("no key code for {0:?}")]
    Unsupported(Key),

    #[error("key code is unknown")]
    UnknownKeyCode,

    #[error("seat sent no xkb keymap")]
    NoSeatKeymap,

    #[error("no output named {0}")]
    OutputNotFound(String),

    #[error("input queue is full ({0} commands pending)")]
    QueueFull(usize),

    #[error("keymap exceeds the protocol size limit")]
    KeymapTooLarge,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn from_worker(error: rikka_worker::Error<Error>) -> Error {
    match error {
        rikka_worker::Error::Spawn(error) => Error::Io(error),
        rikka_worker::Error::Startup(error) => error,
        rikka_worker::Error::Exited => Error::Stopped,
    }
}
