use rikka_wayland_client::Proxy;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no wayland session")]
    NoSession,
    #[error("compositor does not advertise {0}")]
    NoGlobal(&'static str),
    #[error("no wl_seat")]
    NoSeat,
    #[error("no seat named `{0}`")]
    SeatNotFound(String),
    #[error("wayland protocol error: {0}")]
    Protocol(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn missing<P: Proxy>() -> Error {
        Error::NoGlobal(P::INTERFACE.name)
    }
}

impl From<rikka_wayland_client::Error> for Error {
    fn from(error: rikka_wayland_client::Error) -> Error {
        use rikka_wayland_client::Error as Client;
        match error {
            Client::NoDisplay
            | Client::NoRuntimeDir
            | Client::InheritedSocket
            | Client::Connect { .. } => Error::NoSession,
            Client::Io(error) => Error::Io(std::io::Error::new(error.kind(), error)),
            Client::Closed => Error::Io(std::io::ErrorKind::ConnectionAborted.into()),
            Client::Protocol(error) => Error::Protocol(error.to_string()),
            Client::Wire(error) => Error::Protocol(error.to_string()),
        }
    }
}
