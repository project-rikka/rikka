use rikka_bus::{Daemon, DaemonError, Fault};

const MISSING: [&str; 2] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
];

const PERMISSION_DENIED: &str = "org.freedesktop.Accounts.Error.PermissionDenied";

#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("AccountsService is not installed")]
    Missing,
    #[error(transparent)]
    Bus(#[from] Fault),
}

pub type Result<T> = std::result::Result<T, Error>;

rikka_bus::daemon_error!(Error, Daemon::Accounts, names = named);

fn named(name: &str, detail: String) -> Error {
    match name {
        missing if MISSING.contains(&missing) => Error::Missing,
        PERMISSION_DENIED => Error::unauthorized(detail),
        other => Error::from_standard_error_name(other, detail),
    }
}
