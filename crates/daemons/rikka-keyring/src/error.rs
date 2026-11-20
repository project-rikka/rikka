use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to reach the session bus: {0}")]
    Bus(#[from] zbus::Error),
    #[error("failed to ask the session bus: {0}")]
    BusCall(#[from] zbus::fdo::Error),
    #[error("failed to read the keyring folder {path}: {source}")]
    Folder {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read the login keyring: {0}")]
    Login(rikka_keyring_vault::Error),
    #[error("failed to start the session collection: {0}")]
    Session(rikka_keyring_vault::Error),
    #[error("another secret service owns org.freedesktop.secrets")]
    Taken,
}
