#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn thread `{name}`")]
    Thread {
        name: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("thread `{name}` stopped before it began")]
    Exited { name: &'static str },

    #[error(transparent)]
    Sound(#[from] discord_voice::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<Error> for discord_core::Error {
    fn from(error: Error) -> discord_core::Error {
        discord_core::Error::voice(error)
    }
}
