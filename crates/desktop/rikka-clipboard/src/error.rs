use rikka_clipboard_core::Error;

pub(crate) fn stopped(error: rikka_worker::SendError) -> Error {
    match error {
        rikka_worker::SendError::Disconnected => Error::Unavailable("clipboard worker stopped"),
        rikka_worker::SendError::Timeout => Error::Unavailable("clipboard worker timed out"),
        rikka_worker::SendError::Io(error) => Error::Io(error),
    }
}

pub(crate) fn startup(error: rikka_worker::Error<Error>) -> Error {
    match error {
        rikka_worker::Error::Spawn(error) => Error::Io(error),
        rikka_worker::Error::Startup(error) => error,
        rikka_worker::Error::Exited => Error::Unavailable("clipboard thread exited during startup"),
    }
}

pub(crate) fn display(error: rikka_wayland::Error) -> Error {
    match error {
        rikka_wayland::Error::NoGlobal(interface) => Error::Unavailable(interface),
        rikka_wayland::Error::Io(error) => Error::Io(error),
        other => Error::Io(std::io::Error::other(other)),
    }
}
