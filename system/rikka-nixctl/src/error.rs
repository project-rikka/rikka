use rikka_nix_protocol::{OBJECT, SERVICE};

use crate::polkit::ACTION;

pub(crate) const LOGS_DIRECTORY: &str = "LOGS_DIRECTORY";

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("failed to connect to the system bus: {0}")]
    Bus(#[source] zbus::Error),
    #[error("failed to acquire {SERVICE} on the system bus: {0}")]
    NameRequest(#[source] zbus::Error),
    #[error("cannot emit signals from {OBJECT}: {0}")]
    SignalEmitter(#[source] zbus::Error),
    #[error("cannot encode the polkit subject: {0}")]
    Subject(#[source] zvariant::Error),
    #[error("polkit is unavailable: {0}")]
    Polkit(#[source] zbus::Error),
    #[error("polkit authorization check for {ACTION} failed: {0}")]
    Authorization(#[source] zbus::Error),
    #[error("authorization timed out after {minutes} minutes")]
    AuthorizationTimeout { minutes: u64 },
    #[error("${LOGS_DIRECTORY} is not set; run rikka-nixctl through rikka-nixctl.service")]
    LogsDirectory,
    #[error(transparent)]
    Journal(#[from] rikka_nix_journal::Error),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
