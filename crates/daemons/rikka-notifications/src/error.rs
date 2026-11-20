#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("session bus is unreachable: {0}")]
    Unreachable(String),
    #[error("another notification server owns the bus name")]
    Taken,
    #[error("failed to start notification server: {0}")]
    Refused(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Self {
        match &error {
            zbus::Error::NameTaken => Self::Taken,
            zbus::Error::Address(_) | zbus::Error::InputOutput(_) | zbus::Error::Handshake(_) => {
                Self::Unreachable(error.to_string())
            }
            _ => Self::Refused(error.to_string()),
        }
    }
}
