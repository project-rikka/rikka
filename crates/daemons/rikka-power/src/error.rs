use rikka_bus::{Daemon, Fault};

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Bus(#[from] Fault),
    #[error("logind has no session for this process: {0}")]
    Sessionless(String),
    #[error("failed to read backlight: {0}")]
    Backlight(String),
}

rikka_bus::daemon_error!(Error, Daemon::Logind);

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
