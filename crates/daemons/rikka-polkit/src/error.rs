use rikka_bus::{Daemon, Fault};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Bus(#[from] Fault),
    #[error("no polkit subject for this session: {0}")]
    Sessionless(String),
    #[error("failed to start authentication helper: {0}")]
    Helperless(String),
    #[error("authentication helper stopped responding: {0}")]
    Broken(String),
}

rikka_bus::daemon_error!(Error, Daemon::Polkit);

pub type Result<T, E = Error> = std::result::Result<T, E>;
