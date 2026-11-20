use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not a keyring: {reason}")]
    Malformed { path: PathBuf, reason: String },
    #[error("keyring file {path}: {source}")]
    File {
        path: PathBuf,
        source: rikka_file_format::Error,
    },
    #[error("{path} already exists")]
    Exists { path: PathBuf },
    #[error("{path} was replaced by another keyring")]
    Replaced { path: PathBuf },
    #[error("{path} failed authentication")]
    Tampered { path: PathBuf },
    #[error("the keyring is locked")]
    Locked,
    #[error("no item {0} in the keyring")]
    NoSuchItem(u64),
    #[error("failed to draw random bytes: {0}")]
    Random(getrandom::Error),
    #[error("failed to derive a key from the password: {0}")]
    Kdf(String),
    #[error("failed to set up the cipher")]
    Cipher,
    #[error("failed to map key memory: {0}")]
    Memory(std::io::Error),
    #[error("failed to bind the keyring: {0}")]
    Binding(String),
}
