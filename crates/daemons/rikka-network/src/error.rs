use rikka_bus::{Daemon, DaemonError, Fault};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Bus(#[from] Fault),
    #[error("{what}: {detail}")]
    Invalid { what: &'static str, detail: String },
}

impl Error {
    pub(crate) fn invalid(what: &'static str, detail: impl Into<String>) -> Error {
        Error::Invalid {
            what,
            detail: detail.into(),
        }
    }
}

rikka_bus::daemon_error!(Error, Daemon::NetworkManager, names = named);

fn named(name: &str, detail: String) -> Error {
    match name {
        "org.freedesktop.NetworkManager.PermissionDenied"
        | "org.freedesktop.NetworkManager.Settings.PermissionDenied"
        | "org.freedesktop.NetworkManager.Settings.Connection.PermissionDenied" => {
            Error::unauthorized(detail)
        }
        "org.freedesktop.NetworkManager.UnknownDevice"
        | "org.freedesktop.NetworkManager.UnknownConnection"
        | "org.freedesktop.NetworkManager.Settings.InvalidConnection" => Error::gone(),
        _ => Error::from_standard_error_name(name, detail),
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
