#[cfg(unix)]
use rikka_secrets_protocol::names::{IS_LOCKED, NO_SESSION};

#[cfg(unix)]
const NO_OWNER: [&str; 2] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
];

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(unix)]
    #[error("failed to connect to the session bus: {0}")]
    NoBus(zbus::Error),
    #[error("no secret service on the session bus")]
    NoService,
    #[error("the keyring is locked")]
    Locked,
    #[error("unlocking the keyring was dismissed")]
    Dismissed,
    #[error("the secret service has no default collection")]
    NoCollection,
    #[error("secret service failed: {0}")]
    Service(String),
    #[cfg(unix)]
    #[error("secret transport failed: {0}")]
    Transport(#[from] rikka_secrets_protocol::Error),
    #[cfg(windows)]
    #[error("credential manager failed: {0}")]
    Credentials(#[source] std::io::Error),
    #[error("stored secret is not valid JSON for its reader: {0}")]
    Encoding(#[from] serde_json::Error),
}

#[cfg(unix)]
impl Error {
    pub(crate) fn from_call(error: zbus::Error) -> Error {
        match &error {
            zbus::Error::MethodError(name, _, _) if NO_OWNER.contains(&name.as_str()) => {
                Error::NoService
            }
            zbus::Error::MethodError(name, _, _) if name.as_str() == IS_LOCKED => Error::Locked,
            zbus::Error::InputOutput(_) => Error::NoBus(error),
            _ => Error::Service(error.to_string()),
        }
    }

    pub(crate) fn is_lost_session(error: &zbus::Error) -> bool {
        matches!(error, zbus::Error::MethodError(name, _, _) if name.as_str() == NO_SESSION)
    }
}
