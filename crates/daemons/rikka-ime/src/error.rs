#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot connect to session bus: {0}")]
    NoBus(String),
    #[error("input panel name `{0}` is already owned")]
    Taken(&'static str),
    #[error("fcitx5 is not running")]
    NotRunning,
    #[error("fcitx5 call `{call}` failed: {reason}")]
    Refused { call: &'static str, reason: String },
    #[error("unexpected fcitx5 reply to `{call}`: {reason}")]
    Unintelligible { call: &'static str, reason: String },
}

impl Error {
    pub(crate) fn bus(error: impl std::fmt::Display) -> Error {
        Error::NoBus(error.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
