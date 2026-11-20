use rikka_bus::{Daemon, DaemonError, Fault};

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Bus(#[from] Fault),
    #[error("pairing rejected")]
    Rejected,
    #[error("pairing timed out on the remote device")]
    TimedOut,
}

rikka_bus::daemon_error!(Error, Daemon::Bluez, names = named);

fn named(name: &str, detail: String) -> Error {
    match name {
        "org.bluez.Error.Rejected"
        | "org.bluez.Error.AuthenticationRejected"
        | "org.bluez.Error.Canceled"
        | "org.bluez.Error.AuthenticationCanceled" => Error::Rejected,
        "org.bluez.Error.AuthenticationTimeout" | "org.bluez.Error.ConnectionAttemptFailed" => {
            Error::TimedOut
        }
        "org.bluez.Error.DoesNotExist" => Error::gone(),
        "org.bluez.Error.NotAuthorized" => Error::unauthorized(detail),
        _ => Error::from_standard_error_name(name, detail),
    }
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
