use std::io::ErrorKind;

const NO_OWNER: [&str; 2] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
];

const NO_REPLY: [&str; 2] = [
    "org.freedesktop.DBus.Error.NoReply",
    "org.freedesktop.DBus.Error.Timeout",
];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to connect to session bus: {0}")]
    NoSession(zbus::Error),
    #[error("no notification server")]
    NoServer,
    #[error("notification server timed out")]
    Timeout,
    #[error("notification server error: {0}")]
    Server(String),
    #[error("failed to spawn signal listener: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("{width}×{height} pixels do not fit in {bytes} bytes of RGBA")]
    Picture {
        width: u32,
        height: u32,
        bytes: usize,
    },
}

impl Error {
    pub(crate) fn from_call(error: zbus::Error) -> Error {
        match &error {
            zbus::Error::MethodError(name, _, _) if NO_OWNER.contains(&name.as_str()) => {
                Error::NoServer
            }
            zbus::Error::MethodError(name, _, _) if NO_REPLY.contains(&name.as_str()) => {
                Error::Timeout
            }
            zbus::Error::InputOutput(io) if io.kind() == ErrorKind::TimedOut => Error::Timeout,
            zbus::Error::InputOutput(_) => Error::NoSession(error),
            _ => Error::Server(error.to_string()),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
